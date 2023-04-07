use std::sync::Arc;

use re_log_types::LogMsg;

pub fn stream_rrd_from_http_to_channel(url: String) -> re_smart_channel::Receiver<LogMsg> {
    let (tx, rx) = re_smart_channel::smart_channel(re_smart_channel::Source::RrdHttpStream {
        url: url.clone(),
    });
    stream_rrd_from_http(
        url,
        Arc::new(move |msg| {
            tx.send(msg).ok();
        }),
    );
    rx
}

pub fn stream_rrd_from_http(url: String, on_msg: Arc<dyn Fn(LogMsg) + Send + Sync>) {
    re_log::debug!("Downloading .rrd file from {url:?}…");

    // TODO(emilk): stream the http request, progressively decoding the .rrd file.
    ehttp::fetch(ehttp::Request::get(&url), move |result| match result {
        Ok(response) => {
            if response.ok {
                re_log::debug!("Decoding .rrd file from {url:?}…");
                decode_rrd(response.bytes, on_msg);
            } else {
                re_log::error!(
                    "Failed to fetch .rrd file from {url}: {} {}",
                    response.status,
                    response.status_text
                );
            }
        }
        Err(err) => {
            re_log::error!("Failed to fetch .rrd file from {url}: {err}");
        }
    });
}

#[cfg(target_arch = "wasm32")]
mod web_event_listener {
    use js_sys::Uint8Array;
    use re_log_types::LogMsg;
    use std::sync::Arc;
    use wasm_bindgen::{closure::Closure, JsCast, JsValue};
    use web_sys::MessageEvent;

    pub fn stream_rrd_from_event_listener(on_msg: Arc<dyn Fn(LogMsg) + Send>) {
        {
            let window = web_sys::window().expect("no global `window` exists");
            let closure = Closure::wrap(Box::new(move |event: JsValue| {
                match event.dyn_into::<MessageEvent>() {
                    Ok(message_event) => {
                        let uint8_array = Uint8Array::new(&message_event.data());
                        let result: Vec<u8> = uint8_array.to_vec();

                        crate::stream_rrd_from_http::decode_rrd(result, on_msg.clone());
                    }
                    Err(err) => {
                        re_log::error!("Incoming event was not a MessageEvent. {:?}", err);
                    }
                }
            }) as Box<dyn FnMut(_)>);
            window
                .add_event_listener_with_callback("message", closure.as_ref().unchecked_ref())
                .unwrap();
            closure.forget();
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use web_event_listener::stream_rrd_from_event_listener;

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::needless_pass_by_value)] // must match wasm version
fn decode_rrd(rrd_bytes: Vec<u8>, on_msg: Arc<dyn Fn(LogMsg) + Send>) {
    match crate::decoder::Decoder::new(rrd_bytes.as_slice()) {
        Ok(decoder) => {
            for msg in decoder {
                match msg {
                    Ok(msg) => {
                        on_msg(msg);
                    }
                    Err(err) => {
                        re_log::warn_once!("Failed to decode message: {err}");
                    }
                }
            }
        }
        Err(err) => {
            re_log::error!("Failed to decode .rrd: {err}");
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web_decode {
    use re_log_types::LogMsg;
    use std::sync::Arc;

    pub fn decode_rrd(rrd_bytes: Vec<u8>, on_msg: Arc<dyn Fn(LogMsg) + Send>) {
        wasm_bindgen_futures::spawn_local(decode_rrd_async(rrd_bytes, on_msg));
    }

    /// Decodes the file in chunks, with an yield between each chunk.
    ///
    /// This is cooperative multi-tasking.
    async fn decode_rrd_async(rrd_bytes: Vec<u8>, on_msg: Arc<dyn Fn(LogMsg) + Send>) {
        let mut last_yield = instant::Instant::now();

        match crate::decoder::Decoder::new(rrd_bytes.as_slice()) {
            Ok(decoder) => {
                for msg in decoder {
                    match msg {
                        Ok(msg) => {
                            on_msg(msg);
                        }
                        Err(err) => {
                            re_log::warn_once!("Failed to decode message: {err}");
                        }
                    }

                    if last_yield.elapsed() > instant::Duration::from_millis(10) {
                        // yield to the ui task
                        yield_().await;
                        last_yield = instant::Instant::now();
                    }
                }
            }
            Err(err) => {
                re_log::error!("Failed to decode .rrd: {err}");
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
