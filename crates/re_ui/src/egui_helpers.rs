/// Is anything in egui being dragged?
pub fn is_anything_being_dragged(egui_ctx: &egui::Context) -> bool {
    // As soon as a button is down, egui considers it a drag.
    // That is, even a click is considered a drag until it is over.
    // So we need some special treatment here.
    // TODO(emilk): make it easier to distinguish between clicks and drags in egui.

    // copied from egui
    /// If the pointer is down for longer than this, it won't become a click (but it is still a drag)
    const MAX_CLICK_DURATION: f64 = 0.6;
    egui_ctx.input(|i| {
        if let Some(press_start_time) = i.pointer.press_start_time() {
            let held_time = i.time - press_start_time;
            held_time > MAX_CLICK_DURATION || i.pointer.is_moving()
        } else {
            false
        }
    })
}
