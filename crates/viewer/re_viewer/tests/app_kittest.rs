use egui_kittest::{
    Node,
    kittest::{By, Queryable},
};

mod viewer_test_utils;

async fn wait_for<'app, 'harness, Getter>(
    mut getter: Getter,
    harness: &'harness mut egui_kittest::Harness<'app, re_viewer::App>,
) where
    Getter: for<'a> FnMut(&'a egui_kittest::Harness<'app, re_viewer::App>) -> Option<Node<'a>>,
{
    loop {
        match getter(harness) {
            Some(_) => {
                break;
            }
            None => {}
        }
        harness.step();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn main() {
    let mut harness = viewer_test_utils::viewer_harness();
    wait_for(
        |harness| {
            harness
                .query_by_label("Air traffic data")
                .filter(|_| !harness.ctx.has_pending_images())
        },
        &mut harness,
    )
    .await;
    harness.get_by_label("menu").click();
    harness.run();
    harness.get_by_label_contains("Settings…").click();
    wait_for(
        |harness| {
            harness.query_by_label_contains(
                "The specified FFmpeg binary path does not exist or is not a file.",
            )
        },
        &mut harness,
    )
    .await;
    harness.snapshot("test_viewer");
}
