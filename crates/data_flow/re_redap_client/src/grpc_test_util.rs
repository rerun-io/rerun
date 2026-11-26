//! A scripted gRPC server, for testing the client without a network.

use std::collections::VecDeque;
use std::sync::Arc;

use http_body::{Body as _, Frame};
use re_protos::external::prost::bytes::Bytes;

/// A client of a server that answers each call with the next of `responses`, along with the
/// request body of every call the server received.
pub fn scripted_client(
    responses: Vec<http::Response<tonic::body::Body>>,
) -> (crate::ConnectionClient, Arc<re_mutex::Mutex<Vec<Bytes>>>) {
    let responses = Arc::new(re_mutex::Mutex::new(VecDeque::from(responses)));
    let requests = Arc::new(re_mutex::Mutex::new(Vec::new()));
    let service = tower::service_fn({
        let requests = requests.clone();
        move |request: http::Request<tonic::body::Body>| {
            let responses = responses.clone();
            let requests = requests.clone();
            async move {
                let mut body = request.into_body();
                let mut bytes = Vec::new();
                while let Some(frame) =
                    std::future::poll_fn(|cx| std::pin::Pin::new(&mut body).poll_frame(cx)).await
                {
                    if let Ok(data) = frame?.into_data() {
                        bytes.extend_from_slice(&data);
                    }
                }
                requests.lock().push(Bytes::from(bytes));
                let response = responses.lock().pop_front();
                tonic::Result::Ok(response.expect("a scripted response for every call"))
            }
        }
    });
    let client = crate::RedapClient::new(
        re_uri::Origin::test(),
        crate::grpc::boxed_redap_grpc_client(service),
        None,
    );
    (client, requests)
}

/// `message` as an uncompressed gRPC message, with its length prefix.
pub fn grpc_frame(message: &impl re_protos::external::prost::Message) -> Frame<Bytes> {
    let message = message.encode_to_vec();
    let mut data = vec![0];
    data.extend_from_slice(&(message.len() as u32).to_be_bytes());
    data.extend_from_slice(&message);
    Frame::data(Bytes::from(data))
}

pub struct Frames(pub VecDeque<Frame<Bytes>>);

impl http_body::Body for Frames {
    type Data = Bytes;
    type Error = tonic::Status;

    fn poll_frame(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<tonic::Result<Frame<Bytes>>>> {
        std::task::Poll::Ready(self.0.pop_front().map(Ok))
    }
}
