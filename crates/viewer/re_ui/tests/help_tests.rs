use re_ui::help::{ControlItem, ControlRow, Help};
use re_ui::icons;

#[cfg(test)]
fn help_ui() {}

fn example_help() -> Help {
    Help {
        title: "2D View".to_owned(),
        markdown: None,
        docs_link: Some("https://docs.rs/egui/latest/egui/".to_owned()),
        controls: vec![
            ControlRow {
                text: "Pan".to_owned(),
                control: vec![
                    ControlItem::icon(icons::LEFT_MOUSE_CLICK),
                    ControlItem::text("+ drag"),
                ],
            },
            ControlRow {
                text: "Zoom".to_owned(),
                control: vec![
                    ControlItem::text("Ctrl / Cmd + "),
                    ControlItem::icon(icons::SCROLL),
                ],
            },
            ControlRow {
                text: "Reset view".to_owned(),
                control: vec![
                    ControlItem::Text("double".to_owned()),
                    ControlItem::Icon(icons::LEFT_MOUSE_CLICK),
                ],
            },
        ],
    }
}
