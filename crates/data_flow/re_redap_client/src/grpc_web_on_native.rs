use futures::TryStreamExt as _;
use http::{Request, Response, header};
use http_body_util::{BodyExt as _, Full, StreamBody};
use tokio_util::{codec::LengthDelimitedCodec, io::StreamReader};
use tonic::Status;
use tonic::body::Body;
use tower::{Service, ServiceExt as _};

use crate::grpc::{GrpcTransport, check_response};

/// Sends `request` over gRPC-Web.
pub async fn send<S>(connection: S, request: Request<Body>) -> Result<Response<Body>, Status>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Error: Into<tonic::codegen::StdError>,
    S::Future: Send,
{
    // gRPC-Web cannot stream request bodies, so the whole request, including every message of a
    // client stream such as `WriteTable`, is buffered and sent with a Content-Length.
    // Responses remain streamed; gRPC-Web trailers are decoded by tonic-web.
    let (mut parts, body) = request.into_parts();
    let bytes = body.collect().await?.to_bytes();
    parts
        .headers
        .insert(header::CONTENT_LENGTH, bytes.len().into());
    let request = Request::from_parts(parts, Body::new(Full::new(bytes)));
    let transport = tower::service_fn(move |request: Request<tonic_web::GrpcWebCall<Body>>| {
        let connection = connection.clone();
        async move {
            let response = connection.oneshot(request.map(Body::new)).await?;
            Ok::<_, S::Error>(response.map(|body| {
                // tonic-web only decodes the trailer block if it arrives as one complete body
                // frame, but HTTP can split or merge gRPC-Web frames arbitrarily.
                // So re-chunk the body into whole gRPC-Web frames before tonic-web reads it.
                // Each frame is a one-byte flag, a four-byte big-endian payload length, then the
                // payload.
                let frames = LengthDelimitedCodec::builder()
                    // The length comes after the flag byte.
                    .length_field_offset(1)
                    .length_field_length(4)
                    // The length counts only the payload; each frame also has 5 header bytes.
                    .length_adjustment(5)
                    // Keep the header in each frame, because tonic-web decodes it.
                    .num_skip(0)
                    // The default limit is 8 MiB, smaller than the messages we accept.
                    .max_frame_length(crate::MAX_DECODING_MESSAGE_SIZE)
                    .new_read(StreamReader::new(
                        body.into_data_stream().map_err(std::io::Error::other),
                    ))
                    .map_ok(|bytes| http_body::Frame::data(bytes.freeze()));
                Body::new(StreamBody::new(frames))
            }))
        }
    });
    let response = tonic_web::GrpcWebClientService::new(transport)
        .oneshot(request)
        .await
        .map_err(|err| Status::from_error(err.into()))?;
    check_response(response.map(Body::new), GrpcTransport::Web)
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::sync::Arc;

    use re_protos::cloud::v1alpha1::rerun_cloud_service_server::SERVICE_NAME;

    use futures::FutureExt as _;
    use tonic::codegen::Bytes;

    use super::*;
    use crate::grpc::send_request;

    fn request(method: &str) -> Request<Body> {
        Request::builder()
            .uri(format!("/{SERVICE_NAME}/{method}"))
            .method("POST")
            .header(header::CONTENT_TYPE, "application/grpc")
            .body(Body::new(Full::new(Bytes::from_static(b"\0\0\0\0\0"))))
            .unwrap()
    }

    fn response(content_type: &str) -> Response<Body> {
        Response::builder()
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn web_write_sends_all_messages_with_content_length() {
        let connection = tower::service_fn(|request: Request<Body>| async move {
            assert_eq!(
                request.headers()[header::CONTENT_TYPE],
                "application/grpc-web"
            );
            assert_eq!(request.headers()[header::CONTENT_LENGTH], "14");
            assert_eq!(
                request
                    .into_body()
                    .collect()
                    .await
                    .unwrap()
                    .to_bytes()
                    .as_ref(),
                b"\0\0\0\0\x02\x08\x01\0\0\0\0\x02\x08\x02"
            );
            Ok::<_, Infallible>(response("application/grpc-web+proto"))
        });
        let mut write = request("WriteTable");
        let frames = [b"\0\0\0\0\x02\x08\x01", b"\0\0\0\0\x02\x08\x02"]
            .map(|message| Ok::<_, Status>(http_body::Frame::data(Bytes::from_static(message))));
        *write.body_mut() = Body::new(StreamBody::new(futures::stream::iter(frames)));
        send_request(GrpcTransport::Web, connection, write)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn web_trailers_survive_coalesced_and_fragmented_http_bodies() {
        let wire = b"\0\0\0\0\x02\x10\x01\x80\0\0\0\x10grpc-status: 0\r\n";
        for chunk_size in [1, 4, wire.len()] {
            let connection = tower::service_fn(move |_: Request<Body>| async move {
                let frames = wire.chunks(chunk_size).map(|chunk| {
                    Ok::<_, Status>(http_body::Frame::data(Bytes::copy_from_slice(chunk)))
                });
                Ok::<_, Infallible>(
                    Response::builder()
                        .header(header::CONTENT_TYPE, "application/grpc-web+proto")
                        .body(Body::new(StreamBody::new(futures::stream::iter(frames))))
                        .unwrap(),
                )
            });

            let collected = send_request(GrpcTransport::Web, connection, request("WhoAmI"))
                .await
                .unwrap()
                .into_body()
                .collect()
                .await
                .unwrap();

            assert_eq!(collected.trailers().unwrap()["grpc-status"], "0");
            assert_eq!(collected.to_bytes().as_ref(), &wire[..7]);
        }
    }

    #[tokio::test]
    async fn web_responses_stream_before_trailers_arrive() {
        let (mut sender, receiver) = futures::channel::mpsc::unbounded();
        let receiver = Arc::new(re_mutex::Mutex::new(Some(receiver)));
        let connection = tower::service_fn(move |_: Request<Body>| {
            let receiver = receiver.lock().take().unwrap();
            async move {
                Ok::<_, Infallible>(
                    Response::builder()
                        .header(header::CONTENT_TYPE, "application/grpc-web+proto")
                        .body(Body::new(StreamBody::new(receiver)))
                        .unwrap(),
                )
            }
        });
        let mut body = send_request(GrpcTransport::Web, connection, request("WhoAmI"))
            .await
            .unwrap()
            .into_body();
        let message = Bytes::from_static(b"\0\0\0\0\x02\x10\x01");

        sender
            .start_send(Ok::<_, Status>(http_body::Frame::data(message.clone())))
            .unwrap();
        let frame = body
            .frame()
            .now_or_never()
            .expect("message should be readable before the trailers arrive")
            .unwrap()
            .unwrap();
        assert_eq!(frame.into_data().unwrap(), message);

        sender
            .start_send(Ok(http_body::Frame::data(Bytes::from_static(
                b"\x80\0\0\0\x10grpc-status: 7\r\n",
            ))))
            .unwrap();
        drop(sender);
        let trailers = body
            .frame()
            .await
            .unwrap()
            .unwrap()
            .into_trailers()
            .unwrap();
        assert_eq!(trailers["grpc-status"], "7");
    }
}
