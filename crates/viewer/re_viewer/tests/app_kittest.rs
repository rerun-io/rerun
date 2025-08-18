use egui_kittest::kittest::Queryable;

mod viewer_test_utils;

#[tokio::test]
async fn main() {
    let mut harness = viewer_test_utils::viewer_harness();
    loop {
        harness.step();
        if harness.query_by_label("Air traffic data").is_some() && !harness.ctx.has_pending_images()
        {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    harness.snapshot("test_viewer");
}
