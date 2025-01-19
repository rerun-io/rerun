use std::fmt::Display;

use re_log_encoding::protobuf_conversions::log_msg_from_proto;
use re_log_types::LogMsg;
use re_protos::sdk_comms::v0::message_proxy_client::MessageProxyClient;
use re_protos::sdk_comms::v0::Empty;
use tokio_stream::StreamExt;
use url::Url;

use crate::StreamError;
use crate::TonicStatusError;

pub fn stream(
    url: String,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<re_smart_channel::Receiver<LogMsg>, InvalidMessageProxyAddress> {
    re_log::debug!("Loading {url} via gRPC…");

    let parsed_url = MessageProxyAddress::parse(&url)?;

    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::MessageProxy { url: url.clone() },
        re_smart_channel::SmartChannelSource::MessageProxy { url: url.clone() },
    );

    crate::spawn_future(async move {
        if let Err(err) = stream_async(parsed_url, tx, on_msg).await {
            re_log::error!(
                "Error while streaming from {url}: {}",
                re_error::format_ref(&err)
            );
        }
    });

    Ok(rx)
}

struct MessageProxyAddress(String);

impl MessageProxyAddress {
    fn parse(url: &str) -> Result<Self, InvalidMessageProxyAddress> {
        let mut parsed = Url::parse(url).map_err(|err| InvalidMessageProxyAddress {
            url: url.to_owned(),
            msg: err.to_string(),
        })?;

        if !parsed.scheme().starts_with("temp") {
            return Err(InvalidMessageProxyAddress {
                url: url.to_owned(),
                msg: format!(
                    "Invalid scheme {:?}, expected {:?}",
                    parsed.scheme(),
                    "temp"
                ),
            });
        }

        parsed.set_scheme("http").ok();

        Ok(Self(parsed.to_string()))
    }

    fn to_http(&self) -> String {
        self.0.clone()
    }
}

impl Display for MessageProxyAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("invalid message proxy address {url:?}: {msg}")]
pub struct InvalidMessageProxyAddress {
    pub url: String,
    pub msg: String,
}

async fn stream_async(
    url: MessageProxyAddress,
    tx: re_smart_channel::Sender<LogMsg>,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<(), StreamError> {
    let mut client = {
        let url = url.to_http();

        #[cfg(target_arch = "wasm32")]
        let tonic_client = {
            tonic_web_wasm_client::Client::new_with_options(
                url,
                tonic_web_wasm_client::options::FetchOptions::new(),
            )
        };

        #[cfg(not(target_arch = "wasm32"))]
        let tonic_client = { tonic::transport::Endpoint::new(url)?.connect().await? };

        // TODO(#8411): figure out the right size for this
        MessageProxyClient::new(tonic_client).max_decoding_message_size(usize::MAX)
    };

    re_log::debug!("Streaming messages from gRPC endpoint {url}");

    let stream = client
        .read_messages(Empty {})
        .await
        .map_err(TonicStatusError)?
        .into_inner();
    tokio::pin!(stream);

    loop {
        match stream.try_next().await {
            Ok(Some(msg)) => {
                let msg = log_msg_from_proto(msg)?;
                if tx.send(msg).is_err() {
                    re_log::debug!("gRPC stream smart channel closed");
                    break;
                }
                if let Some(on_msg) = &on_msg {
                    on_msg();
                }
            }

            // Stream closed
            Ok(None) => {
                re_log::debug!("gRPC stream disconnected");
                break;
            }

            Err(_) => {
                re_log::debug!("gRPC stream timed out");
                break;
            }
        }
    }

    Ok(())
}
