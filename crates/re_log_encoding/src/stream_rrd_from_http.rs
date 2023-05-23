use std::sync::Arc;

use re_error::ResultExt as _;
use re_log_types::LogMsg;

pub fn stream_rrd_from_http_to_channel(url: String) -> re_smart_channel::Receiver<LogMsg> {
    let (tx, rx) = re_smart_channel::smart_channel(
        re_smart_channel::SmartMessageSource::RrdHttpStream { url: url.clone() },
        re_smart_channel::SmartChannelSource::RrdHttpStream { url: url.clone() },
    );
    stream_rrd_from_http(
        url,
        Arc::new(move |msg| {
            match msg {
                HttpMessage::LogMsg(msg) => tx.send(msg).warn_on_err_once("failed to send message"),
                HttpMessage::Success => {
                    tx.quit(None).warn_on_err_once("failed to send quit marker")
                }
                HttpMessage::Failure(err) => tx
                    .quit(Some(err))
                    .warn_on_err_once("failed to send quit marker"),
            };
        }),
    );
    rx
}

/// An intermediate message when decoding an rrd file fetched over HTTP.
pub enum HttpMessage {
    /// The next [`LogMsg`] in the decoding stream.
    LogMsg(LogMsg),

    /// Everything has been successfully decoded. End of stream.
    Success,

    /// Something went wrong. End of stream.
    Failure(Box<dyn std::error::Error + Send + Sync>),
}

pub type HttpMessageCallback = dyn Fn(HttpMessage) + Send + Sync;

pub fn stream_rrd_from_http(url: String, on_msg: Arc<HttpMessageCallback>) {
    re_log::debug!("Downloading .rrd file from {url:?}…");

    // TODO(emilk): stream the http request, progressively decoding the .rrd file.
    ehttp::fetch(ehttp::Request::get(&url), move |result| match result {
        Ok(response) => {
            if response.ok {
                re_log::debug!("Decoding .rrd file from {url:?}…");
                decode_rrd(response.bytes, on_msg);
            } else {
                let err = format!(
                    "Failed to fetch .rrd file from {url}: {} {}",
                    response.status, response.status_text
                );
                on_msg(HttpMessage::Failure(err.into()));
            }
        }
        Err(err) => {
            on_msg(HttpMessage::Failure(
                format!("Failed to fetch .rrd file from {url}: {err}").into(),
            ));
        }
    });
}

#[cfg(target_arch = "wasm32")]
mod web_event_listener {
    use super::HttpMessageCallback;
    use js_sys::Uint8Array;
    use std::sync::Arc;
    use wasm_bindgen::{closure::Closure, JsCast, JsValue};
    use web_sys::MessageEvent;

    /// Install an event-listener on `window` which will decode the incoming event as an rrd
    ///
    /// From javascript you can send an rrd using:
    /// ``` ignore
    /// var rrd = new Uint8Array(...); // Get an RRD from somewhere
    /// window.postMessage(rrd, "*")
    /// ```
    pub fn stream_rrd_from_event_listener(on_msg: Arc<HttpMessageCallback>) {
        let window = web_sys::window().expect("no global `window` exists");
        let closure = Closure::wrap(Box::new({
            move |event: JsValue| match event.dyn_into::<MessageEvent>() {
                Ok(message_event) => {
                    let uint8_array = Uint8Array::new(&message_event.data());
                    let result: Vec<u8> = uint8_array.to_vec();
                    crate::stream_rrd_from_http::decode_rrd(result, Arc::clone(&on_msg));
                }
                Err(js_val) => {
                    re_log::error!("Incoming event was not a MessageEvent. {:?}", js_val);
                }
            }
        }) as Box<dyn FnMut(_)>);
        window
            .add_event_listener_with_callback("message", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }
}

#[cfg(target_arch = "wasm32")]
pub use web_event_listener::stream_rrd_from_event_listener;

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::needless_pass_by_value)] // must match wasm version
fn decode_rrd(rrd_bytes: Vec<u8>, on_msg: Arc<HttpMessageCallback>) {
    match crate::decoder::Decoder::new(rrd_bytes.as_slice()) {
        Ok(decoder) => {
            for msg in decoder {
                match msg {
                    Ok(msg) => {
                        on_msg(HttpMessage::LogMsg(msg));
                    }
                    Err(err) => {
                        re_log::warn_once!("Failed to decode message: {err}");
                    }
                }
            }
            on_msg(HttpMessage::Success);
        }
        Err(err) => {
            on_msg(HttpMessage::Failure(
                format!("Failed to decode .rrd: {err}").into(),
            ));
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web_decode {
    use super::{HttpMessage, HttpMessageCallback};
    use std::sync::Arc;

    pub fn decode_rrd(rrd_bytes: Vec<u8>, on_msg: Arc<HttpMessageCallback>) {
        wasm_bindgen_futures::spawn_local(decode_rrd_async(rrd_bytes, on_msg));
    }

    /// Decodes the file in chunks, with an yield between each chunk.
    ///
    /// This is cooperative multi-tasking.
    async fn decode_rrd_async(rrd_bytes: Vec<u8>, on_msg: Arc<HttpMessageCallback>) {
        let mut last_yield = web_time::Instant::now();

        match crate::decoder::Decoder::new(rrd_bytes.as_slice()) {
            Ok(decoder) => {
                for msg in decoder {
                    match msg {
                        Ok(msg) => {
                            on_msg(HttpMessage::LogMsg(msg));
                        }
                        Err(err) => {
                            re_log::warn_once!("Failed to decode message: {err}");
                        }
                    }

                    on_msg(HttpMessage::Success);

                    if last_yield.elapsed() > web_time::Duration::from_millis(10) {
                        // yield to the ui task
                        yield_().await;
                        last_yield = web_time::Instant::now();
                    }
                }
            }
            Err(err) => {
                on_msg(HttpMessage::Failure(
                    format!("Failed to decode .rrd: {err}").into(),
                ));
            }
        }
    }

    // Yield to other tasks
    async fn yield_() {
        // TODO(emilk): create a better async yield function. See https://github.com/rustwasm/wasm-bindgen/issues/3359
        sleep_ms(1).await;
    }

    // Hack to get async sleep on wasm
    async fn sleep_ms(millis: i32) {
        let mut cb = |resolve: js_sys::Function, _reject: js_sys::Function| {
            web_sys::window()
                .unwrap()
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, millis)
                .expect("Failed to call set_timeout");
        };
        let p = js_sys::Promise::new(&mut cb);
        wasm_bindgen_futures::JsFuture::from(p).await.unwrap();
    }
}

#[cfg(target_arch = "wasm32")]
use web_decode::decode_rrd;
