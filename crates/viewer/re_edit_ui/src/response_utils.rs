/// Forwards the changed state from the inner response to the outer response and returns it.
pub fn response_with_changes_of_inner(
    mut inner_response: egui::InnerResponse<Option<egui::Response>>,
) -> egui::Response {
    if let Some(inner) = inner_response.inner {
        if inner.changed() {
            inner_response.response.mark_changed();
        }
    }
    inner_response.response
}
