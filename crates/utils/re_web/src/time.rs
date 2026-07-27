//! Browser timers.

use std::time::Duration;

/// Waits for at least `duration` using the browser's timer queue.
///
/// Browser timers use signed 32-bit millisecond delays, so longer durations are clamped.
pub async fn sleep(duration: Duration) {
    let milliseconds = i32::try_from(duration.as_millis()).unwrap_or(i32::MAX);
    crate::task::spawn_local_with_result(async move {
        let mut callback = |resolve: js_sys::Function, _reject: js_sys::Function| {
            web_sys::window()
                .expect("browser window should exist")
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, milliseconds)
                .expect("browser timer should be created");
        };

        js_sys::Promise::new(&mut callback)
            .await
            .expect("browser timer should complete");
    })
    .await
    .expect("browser timer task should not be canceled while it is awaited");
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn sleeps() {
        super::sleep(Duration::ZERO).await;
    }
}
