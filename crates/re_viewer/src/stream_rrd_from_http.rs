pub fn stream_rrd_from_http_to_channel(
    url: String,
) -> re_smart_channel::Receiver<re_log_types::LogMsg> {
    let (tx, rx) = re_smart_channel::smart_channel(re_smart_channel::Source::RrdHttpStream {
        url: url.clone(),
    });
    stream_rrd_from_http(
        url,
        Box::new(move |msg| {
            tx.send(msg).ok();
        }),
    );
    rx
}

pub fn stream_rrd_from_http(url: String, on_msg: Box<dyn Fn(re_log_types::LogMsg) + Send>) {
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

#[cfg(not(target_arch = "wasm32"))]
#[allow(clippy::needless_pass_by_value)] // must match wasm version
fn decode_rrd(rrd_bytes: Vec<u8>, on_msg: Box<dyn Fn(re_log_types::LogMsg) + Send>) {
    match re_log_types::encoding::Decoder::new(rrd_bytes.as_slice()) {
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
    pub fn decode_rrd(rrd_bytes: Vec<u8>, on_msg: Box<dyn Fn(re_log_types::LogMsg) + Send>) {
        wasm_bindgen_futures::spawn_local(decode_rrd_async(rrd_bytes, on_msg));
    }

    /// Decodes the file in chunks, with an yield between each chunk.
    ///
    /// This is cooperative multi-tasking.
    async fn decode_rrd_async(
        rrd_bytes: Vec<u8>,
        on_msg: Box<dyn Fn(re_log_types::LogMsg) + Send>,
    ) {
        let mut last_yield = instant::Instant::now();

        match re_log_types::encoding::Decoder::new(rrd_bytes.as_slice()) {
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
        sleep_ms(1).await; // TODO(emilk): create a better async yield function
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
