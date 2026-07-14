use std::cell::RefCell;
use std::ops::ControlFlow;
use std::sync::Arc;

use re_log_types::LogMsg;

/// An intermediate message when decoding an rrd file fetched over HTTP.
pub enum HttpMessage {
    /// The next [`LogMsg`] in the decoding stream.
    LogMsg(LogMsg),

    /// Everything has been successfully decoded. End of stream.
    Success,

    /// Something went wrong. End of stream.
    Failure(Error),
}

/// Error type for HTTP streaming failures.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to fetch .rrd file: {status} {status_text}, url: {url}")]
    HttpStatus {
        status: u16,
        status_text: String,
        url: String,
    },

    #[error("Failed to fetch .rrd file: {reason}, url: {url}")]
    Fetch { reason: String, url: String },

    #[error("Failed to decode .rrd file: {source}, url: {url}")]
    Decode {
        #[source]
        source: crate::DecodeError,
        url: String,
    },

    #[error("Failed to decode .rrd: {0}")]
    DecodeEager(#[source] crate::DecodeError),
}

pub type HttpMessageCallback = dyn Fn(HttpMessage) -> ControlFlow<()> + Send + Sync;

pub fn stream_from_http(url: String, on_msg: Arc<HttpMessageCallback>) {
    re_log::debug!("Downloading .rrd file from {url:?}…");

    ehttp::streaming::fetch(ehttp::Request::get(&url), {
        let decoder = RefCell::new(crate::Decoder::new());
        move |part| match part {
            Ok(part) => match part {
                ehttp::streaming::Part::Response(ehttp::PartialResponse {
                    url,
                    ok,
                    status,
                    status_text,
                    headers,
                }) => {
                    re_log::trace!("{url} status: {status} - {status_text}");
                    re_log::trace!("{url} headers: {headers:#?}");
                    if ok {
                        re_log::debug!("Decoding .rrd file from {url:?}…");
                        ControlFlow::Continue(())
                    } else {
                        on_msg(HttpMessage::Failure(Error::HttpStatus {
                            status,
                            status_text,
                            url,
                        }))
                    }
                }
                ehttp::streaming::Part::Chunk(chunk) => {
                    if chunk.is_empty() {
                        re_log::debug!("Finished decoding .rrd file from {url:?}…");
                        return on_msg(HttpMessage::Success);
                    }

                    re_tracing::profile_scope!("decoding_rrd_stream");
                    decoder.borrow_mut().push_byte_chunk(chunk);
                    loop {
                        match decoder.borrow_mut().try_read() {
                            Ok(message) => match message {
                                Some(message) => {
                                    // only return if the callback asks us to
                                    if on_msg(HttpMessage::LogMsg(message)).is_break() {
                                        return ControlFlow::Break(());
                                    }
                                }
                                None => return ControlFlow::Continue(()),
                            },
                            Err(err) => {
                                return on_msg(HttpMessage::Failure(Error::Decode {
                                    source: err,
                                    url: url.clone(),
                                }));
                            }
                        }
                    }
                }
            },
            Err(err) => on_msg(HttpMessage::Failure(Error::Fetch {
                reason: err,
                url: url.clone(),
            })),
        }
    });
}

#[cfg(target_arch = "wasm32")]
// TODO(#3408): remove unwrap()
#[expect(clippy::unwrap_used)]
mod web_event_listener {
    use std::sync::Arc;

    use js_sys::Uint8Array;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast as _, JsValue};
    use web_sys::MessageEvent;

    use super::HttpMessageCallback;

    /// Install an event-listener on `window` which will decode the incoming event as an rrd
    ///
    /// From javascript you can send an rrd using:
    /// ``` ignore
    /// var rrd = new Uint8Array(…); // Get an RRD from somewhere
    /// window.postMessage(rrd, "*")
    /// ```
    pub fn stream_rrd_from_event_listener(on_msg: Arc<HttpMessageCallback>) {
        let window = web_sys::window().expect("no global `window` exists");
        let closure = Closure::wrap(Box::new({
            move |event: JsValue| match event.dyn_into::<MessageEvent>() {
                Ok(message_event) => {
                    let uint8_array = Uint8Array::new(&message_event.data());
                    let result: Vec<u8> = uint8_array.to_vec();
                    crate::rrd::stream_from_http::decode_rrd(result, Arc::clone(&on_msg));
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

#[cfg(target_arch = "wasm32")]
// TODO(#3408): remove unwrap()
#[expect(clippy::unwrap_used)]
pub mod web_decode {
    use std::sync::Arc;

    use super::{Error, HttpMessage, HttpMessageCallback};

    pub fn decode_rrd(rrd_bytes: Vec<u8>, on_msg: Arc<HttpMessageCallback>) {
        wasm_bindgen_futures::spawn_local(decode_rrd_async(rrd_bytes, on_msg));
    }

    /// Decodes the file in chunks, with an yield between each chunk.
    ///
    /// This is cooperative multi-tasking.
    async fn decode_rrd_async(rrd_bytes: Vec<u8>, on_msg: Arc<HttpMessageCallback>) {
        let mut last_yield = web_time::Instant::now();

        match crate::Decoder::decode_eager(rrd_bytes.as_slice()) {
            Ok(decoder) => {
                for msg in decoder {
                    match msg {
                        Ok(msg) => {
                            if on_msg(HttpMessage::LogMsg(msg)).is_break() {
                                return;
                            }
                        }
                        Err(err) => {
                            re_log::warn_once!("Failed to decode message: {err}");
                        }
                    }

                    if on_msg(HttpMessage::Success).is_break() {
                        return;
                    }

                    if last_yield.elapsed() > web_time::Duration::from_millis(10) {
                        // yield to the ui task
                        yield_().await;
                        last_yield = web_time::Instant::now();
                    }
                }
            }
            Err(err) => {
                // Regardless of what the message handler returns, we are done here.
                let _ignored_control_flow = on_msg(HttpMessage::Failure(Error::DecodeEager(err)));
            }
        }
    }

    // Yield to other tasks
    async fn yield_() {
        // TODO(emilk): create a better async yield function. See https://github.com/wasm-bindgen/wasm-bindgen/issues/3359
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
