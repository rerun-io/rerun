use egui::{Color32, RichText};
use re_agent::acp::schema::v1::{ContentBlock, Diff, ToolCallContent, ToolCallStatus, ToolKind};
use re_ui::{UiExt as _, icons};

use re_agent::ToolCallState;

/// A collapsible card for one tool call: status, kind, title, and the details underneath.
pub fn tool_call_ui(ui: &mut egui::Ui, call: &ToolCallState) {
    let id = ui.id().with(&call.id.0);
    let default_open = call.status == ToolCallStatus::Failed;

    let state = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        id,
        default_open,
    );
    let is_open = state.is_open();
    state
        .show_header(ui, |ui| {
            header_ui(ui, call, is_open);
        })
        .body_unindented(|ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| body_ui(ui, call));
        });
}

/// The title is truncated while collapsed and wraps when expanded, so no tooltip is needed.
fn header_ui(ui: &mut egui::Ui, call: &ToolCallState, is_open: bool) {
    ui.spacing_mut().item_spacing.x = 6.0;

    let tokens = ui.tokens();
    match call.status {
        ToolCallStatus::Pending | ToolCallStatus::InProgress => {
            ui.inline_loading_indicator("");
        }
        ToolCallStatus::Completed => {
            ui.small_icon(&icons::SUCCESS, Some(tokens.success_text_color));
        }
        ToolCallStatus::Failed => {
            ui.small_icon(&icons::ERROR, Some(tokens.error_fg_color));
        }
        _ => {}
    }

    if let Some(icon) = kind_icon(call.kind) {
        ui.small_icon(icon, Some(tokens.text_subdued));
    }

    let title = if call.title.is_empty() {
        kind_name(call.kind).to_owned()
    } else {
        call.title.clone()
    };
    let wrap_mode = if is_open {
        egui::TextWrapMode::Wrap
    } else {
        egui::TextWrapMode::Truncate
    };
    ui.add(
        egui::Label::new(RichText::new(title).monospace())
            .wrap_mode(wrap_mode)
            .show_tooltip_when_elided(false)
            .selectable(false),
    );
}

fn body_ui(ui: &mut egui::Ui, call: &ToolCallState) {
    let tokens = ui.tokens();

    if !call.locations.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for location in &call.locations {
                let mut text = location.path.display().to_string();
                if let Some(line) = location.line {
                    text = format!("{text}:{line}");
                }
                ui.label(RichText::new(text).monospace().color(tokens.text_subdued));
            }
        });
    }

    for (index, content) in call.content.iter().enumerate() {
        match content {
            ToolCallContent::Content(content) => {
                content_block_ui(ui, &content.content, ui.id().with(index));
            }
            ToolCallContent::Diff(diff) => diff_ui(ui, diff),
            ToolCallContent::Terminal(terminal) => {
                ui.label(
                    RichText::new(format!("Terminal {}", terminal.terminal_id.0))
                        .color(tokens.text_subdued),
                );
            }
            _ => {}
        }
    }

    if let Some(raw_input) = &call.raw_input {
        raw_json_ui(ui, "Input", raw_input);
    }
    if let Some(raw_output) = &call.raw_output {
        raw_json_ui(ui, "Output", raw_output);
    }
}

fn raw_json_ui(ui: &mut egui::Ui, label: &str, value: &serde_json::Value) {
    egui::CollapsingHeader::new(RichText::new(label).small())
        .id_salt(label)
        .default_open(false)
        .show(ui, |ui| {
            tool_input_ui(ui, value);
        });
}

/// Fields whose text is Markdown meant for a human, e.g. the plan an agent asks approval for.
const MARKDOWN_FIELDS: &[&str] = &[
    "content",
    "description",
    "message",
    "plan",
    "prompt",
    "summary",
    "text",
];

/// Strings longer than this get their own collapsing header instead of an inline line.
const MAX_INLINE_CHARS: usize = 120;

/// A tool call's input, laid out for reading rather than dumped as JSON.
///
/// Objects get one line per field. Long text fields fold under a header and render as Markdown
/// (see [`MARKDOWN_FIELDS`]) or code; fields holding a file path become links that open the file.
/// Anything else is pretty-printed JSON.
pub fn tool_input_ui(ui: &mut egui::Ui, value: &serde_json::Value) {
    let tokens = ui.tokens();
    match value {
        serde_json::Value::String(text) => code_ui(ui, text, None),
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                match value {
                    serde_json::Value::String(text) if is_file_path(text) => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("{key}:")).color(tokens.text_subdued));
                            ui.hyperlink_to(RichText::new(text).monospace(), file_url(text))
                                .on_hover_text("Open the file");
                        });
                    }
                    serde_json::Value::String(text)
                        if text.contains('\n') || MAX_INLINE_CHARS < text.chars().count() =>
                    {
                        let lines = text.lines().count();
                        let header = format!("{key} ({lines} lines)");
                        egui::CollapsingHeader::new(RichText::new(header).small())
                            .id_salt(key)
                            .default_open(false)
                            .show(ui, |ui| {
                                if MARKDOWN_FIELDS.contains(&key.as_str()) {
                                    markdown_ui(ui, text);
                                } else {
                                    code_ui(ui, text, None);
                                }
                            });
                    }
                    serde_json::Value::String(text) => {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(format!("{key}:")).color(tokens.text_subdued));
                            ui.add(
                                egui::Label::new(RichText::new(text).monospace()).selectable(true),
                            );
                        });
                    }
                    serde_json::Value::Null
                    | serde_json::Value::Bool(_)
                    | serde_json::Value::Number(_) => {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(format!("{key}:")).color(tokens.text_subdued));
                            ui.label(RichText::new(value.to_string()).monospace());
                        });
                    }
                    serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                        let json = serde_json::to_string_pretty(value).unwrap_or_default();
                        egui::CollapsingHeader::new(RichText::new(key).small())
                            .id_salt(key)
                            .default_open(false)
                            .show(ui, |ui| code_ui(ui, &json, None));
                    }
                }
            }
        }
        other => {
            let json = serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string());
            code_ui(ui, &json, None);
        }
    }
}

/// An absolute or home-relative path on one line, without spaces that would make it a sentence.
fn is_file_path(text: &str) -> bool {
    let looks_absolute = text.starts_with('/')
        || text.starts_with("~/")
        || text.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && text[1..].starts_with(":\\");
    looks_absolute && !text.contains(char::is_whitespace) && text.len() < 512
}

/// `file://` URL for a path, with `~` expanded so the OS can open it.
fn file_url(path: &str) -> String {
    let path = match path.strip_prefix("~/") {
        Some(rest) => match std::env::var("HOME") {
            Ok(home) => format!("{home}/{rest}"),
            Err(_) => path.to_owned(),
        },
        None => path.to_owned(),
    };
    format!("file://{path}")
}

/// Markdown where each table sits in its own horizontal scroll area, so a wide table scrolls
/// instead of widening the transcript and re-wrapping everything below it.
///
/// TODO(emilk): render plainly once we ship an `egui_commonmark` with
/// <https://github.com/lampsitter/egui_commonmark/pull/103>, which does the same internally.
pub fn markdown_ui(ui: &mut egui::Ui, markdown: &str) {
    for (index, segment) in split_at_tables(markdown).into_iter().enumerate() {
        match segment {
            MarkdownSegment::Text(text) => ui.markdown_ui(text),
            MarkdownSegment::Table(table) => {
                egui::ScrollArea::horizontal()
                    .id_salt(("markdown_table", index))
                    .auto_shrink([false, true])
                    .show(ui, |ui| ui.markdown_ui(table));
            }
        }
    }
}

/// A run of Markdown that is either one table or everything between tables.
#[derive(Debug, PartialEq, Eq)]
enum MarkdownSegment<'a> {
    Text(&'a str),
    Table(&'a str),
}

/// Splits `markdown` so that every table is its own segment.
fn split_at_tables(markdown: &str) -> Vec<MarkdownSegment<'_>> {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

    let mut segments = Vec::new();
    let mut cursor = 0;
    let mut table_start = None;
    for (event, range) in Parser::new_ext(markdown, Options::ENABLE_TABLES).into_offset_iter() {
        match event {
            Event::Start(Tag::Table(_)) => table_start = Some(range.start),
            Event::End(TagEnd::Table) => {
                if let Some(start) = table_start.take() {
                    if cursor < start && !markdown[cursor..start].trim().is_empty() {
                        segments.push(MarkdownSegment::Text(&markdown[cursor..start]));
                    }
                    segments.push(MarkdownSegment::Table(&markdown[start..range.end]));
                    cursor = range.end;
                }
            }
            _ => {}
        }
    }
    if cursor < markdown.len() && !markdown[cursor..].trim().is_empty() {
        segments.push(MarkdownSegment::Text(&markdown[cursor..]));
    }
    segments
}

/// Renders a content block coming from a tool result or an agent message.
pub fn content_block_ui(ui: &mut egui::Ui, block: &ContentBlock, id: egui::Id) {
    match block {
        ContentBlock::Text(text) => {
            markdown_ui(ui, &super::linkify::linkify_bare_urls(&text.text));
        }
        ContentBlock::Image(image) => {
            use base64::Engine as _;
            match base64::engine::general_purpose::STANDARD.decode(&image.data) {
                Ok(bytes) => {
                    let uri = format!("bytes://{}.{}", id.short_debug_format(), &image.mime_type);
                    ui.add(
                        egui::Image::from_bytes(uri, bytes)
                            .max_width(ui.available_width().min(800.0))
                            .corner_radius(4),
                    );
                }
                Err(err) => {
                    ui.error_label(format!("Failed to decode image: {err}"));
                }
            }
        }
        ContentBlock::ResourceLink(link) => {
            ui.hyperlink_to(link.name.clone(), link.uri.clone());
        }
        ContentBlock::Resource(_) => {
            ui.weak("(embedded resource)");
        }
        _ => {
            ui.weak("(unsupported content)");
        }
    }
}

fn diff_ui(ui: &mut egui::Ui, diff: &Diff) {
    let tokens = ui.tokens();
    ui.label(
        RichText::new(diff.path.display().to_string())
            .monospace()
            .color(tokens.text_subdued),
    );

    let removed = tokens.error_fg_color.gamma_multiply(0.15);
    let added = tokens.success_text_color.gamma_multiply(0.15);

    if let Some(old_text) = &diff.old_text
        && !old_text.is_empty()
    {
        code_ui(ui, old_text, Some(removed));
    }
    code_ui(ui, &diff.new_text, Some(added));
}

/// Read-only monospace block that keeps long lines scrollable instead of wrapping the whole panel.
pub fn code_ui(ui: &mut egui::Ui, text: &str, fill: Option<Color32>) {
    let tokens = ui.tokens();
    egui::Frame::new()
        .fill(fill.unwrap_or(tokens.extreme_bg_color))
        .corner_radius(4)
        .inner_margin(6)
        .show(ui, |ui| {
            egui::ScrollArea::horizontal()
                .id_salt(ui.id().with("code"))
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(RichText::new(text).monospace())
                            .wrap_mode(egui::TextWrapMode::Extend)
                            .selectable(true),
                    );
                });
        });
}

fn kind_icon(kind: ToolKind) -> Option<&'static re_ui::Icon> {
    let icon = match kind {
        ToolKind::Read => &icons::VIEW_TEXT,
        ToolKind::Edit => &icons::EDIT,
        ToolKind::Delete => &icons::TRASH,
        ToolKind::Move => &icons::DND_MOVE,
        ToolKind::Search => &icons::SEARCH,
        ToolKind::Execute => &icons::VIEW_LOG,
        ToolKind::Think => &icons::INFO,
        ToolKind::Fetch => &icons::URL,
        ToolKind::SwitchMode => &icons::SETTINGS,
        _ => return None,
    };
    Some(icon)
}

fn kind_name(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::Read => "Read",
        ToolKind::Edit => "Edit",
        ToolKind::Delete => "Delete",
        ToolKind::Move => "Move",
        ToolKind::Search => "Search",
        ToolKind::Execute => "Execute",
        ToolKind::Think => "Think",
        ToolKind::Fetch => "Fetch",
        ToolKind::SwitchMode => "Switch mode",
        _ => "Tool call",
    }
}

#[cfg(test)]
mod tests {
    use super::{MarkdownSegment, split_at_tables};

    #[test]
    fn tables_get_their_own_segment() {
        assert_eq!(split_at_tables(""), vec![]);
        assert_eq!(
            split_at_tables("just text"),
            vec![MarkdownSegment::Text("just text")]
        );

        let markdown =
            "Intro\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nMiddle\n\n| c |\n|---|\n| 3 |\n";
        assert_eq!(
            split_at_tables(markdown),
            vec![
                MarkdownSegment::Text("Intro\n\n"),
                MarkdownSegment::Table("| a | b |\n|---|---|\n| 1 | 2 |\n"),
                MarkdownSegment::Text("\nMiddle\n\n"),
                MarkdownSegment::Table("| c |\n|---|\n| 3 |\n"),
            ]
        );

        // A pipe in prose is not a table.
        assert_eq!(
            split_at_tables("a | b\n"),
            vec![MarkdownSegment::Text("a | b\n")]
        );
    }
}
