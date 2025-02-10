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
    url: &str,
    on_msg: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<re_smart_channel::Receiver<LogMsg>, InvalidMessageProxyUrl> {
    re_log::debug!("Loading {url} via gRPC…");

    let parsed_url = MessageProxyUrl::parse(url)?;

    let url = url.to_owned();
    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::MessageProxy { url: url.clone() },
        re_smart_channel::SmartChannelSource::MessageProxy { url },
    );

    crate::spawn_future(async move {
        if let Err(err) = stream_async(parsed_url, &tx, on_msg).await {
            tx.quit(Some(Box::new(err))).ok();
        }
    });

    Ok(rx)
}

/// Represents a URL to a gRPC server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageProxyUrl(String);

impl MessageProxyUrl {
    /// Parses as a regular URL, the protocol must be `temp://`, `http://`, or `https://`.
    pub fn parse(url: &str) -> Result<Self, InvalidMessageProxyUrl> {
        if url.starts_with("http") {
            let _ = Url::parse(url).map_err(|err| InvalidMessageProxyUrl {
                url: url.to_owned(),
                msg: err.to_string(),
            })?;

            Ok(Self(url.to_owned()))
        }
        // TODO(#8761): URL prefix
        else if let Some(url) = url.strip_prefix("temp") {
            let url = format!("http{url}");

            let _ = Url::parse(&url).map_err(|err| InvalidMessageProxyUrl {
                url: url.clone(),
                msg: err.to_string(),
            })?;

            Ok(Self(url))
        } else {
            let scheme = url.split_once("://").map(|(a, _)| a).ok_or("unknown");

            Err(InvalidMessageProxyUrl {
                url: url.to_owned(),
                msg: format!("Invalid scheme {scheme:?}, expected {:?}", "temp"),
            })
        }
    }

    pub fn to_http(&self) -> String {
        self.0.clone()
    }
}

impl Display for MessageProxyUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl std::str::FromStr for MessageProxyUrl {
    type Err = InvalidMessageProxyUrl;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("invalid message proxy url {url:?}: {msg}")]
pub struct InvalidMessageProxyUrl {
    pub url: String,
    pub msg: String,
}

async fn stream_async(
    url: MessageProxyUrl,
    tx: &re_smart_channel::Sender<LogMsg>,
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

    let mut stream = client
        .read_messages(Empty {})
        .await
        .map_err(TonicStatusError)?
        .into_inner();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_url() {
        struct Case {
            input: &'static str,
            expected: &'static str,
        }
        let cases = [
            Case {
                input: "temp://127.0.0.1:9876",
                expected: "http://127.0.0.1:9876",
            },
            Case {
                input: "http://127.0.0.1:9876",
                expected: "http://127.0.0.1:9876",
            },
        ];

        let mut failed = false;
        for Case { input, expected } in cases {
            let actual = MessageProxyUrl::parse(input).map(|v| v.to_http());
            if actual != Ok(expected.to_owned()) {
                eprintln!("expected {input:?} to parse as {expected:?}, got {actual:?} instead");
                failed = true;
            }
        }
        assert!(!failed, "one or more test cases failed");
    }
}
