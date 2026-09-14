//! Screenshot test for the "smart link" URL buttons produced by the real viewer decorator.
//!
//! The unit tests in `link_button.rs` only check the button *text*; this exercises the full
//! `make_url_decorator` → `re_ui` rendering path across the different built-in URL types, so we
//! catch regressions in icon/label/breadcrumb layout.

#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;

use re_log_types::{EntryId, EntryName};
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_ui::{UiLayout, UrlDecorator};
use re_viewer_context::{LinkKind, ResolvedEntry, UrlNameLookup, make_url_decorator};

const DATASET_TUID: &str = "1830B33B45B963E7774455beb91701ae";
const DATASET_ENTRY_TUID: &str = "9a3c5e7b1d2f486a0b4c6d8e0f123456";
const TABLE_ENTRY_TUID: &str = "182755B45B963E7774455beb91701aef";

/// All the built-in URL types we recognize, paired with a short description.
///
/// Note: a bare local file path (e.g. `/path/to/file.rrd`) is intentionally absent — `data_label`
/// only treats text containing `://` as a URL, so the decorator is never invoked for it.
const URLS: &[(&str, &str)] = &[
    (
        "Dataset segment (resolved)",
        "rerun://example.rerun.io/dataset/1830B33B45B963E7774455beb91701ae?segment_id=segment-abc-12",
    ),
    (
        "Dataset segment (unresolved)",
        "rerun://example.rerun.io/dataset/abcdef0123456789abcdef0123456789?segment_id=seg99",
    ),
    (
        "Dataset entry (resolved)",
        "rerun://example.rerun.io/entry/9a3c5e7b1d2f486a0b4c6d8e0f123456",
    ),
    (
        // Unresolved entries carry no kind, so this falls back to a dataset icon + short id — it
        // renders identically to the "Table entry (unresolved)" row below.
        "Dataset entry (unresolved)",
        "rerun://example.rerun.io/entry/fedcba9876543210fedcba9876543210",
    ),
    (
        "Table entry (resolved)",
        "rerun://example.rerun.io/entry/182755B45B963E7774455beb91701aef",
    ),
    (
        // Same id-only fallback as the unresolved dataset entry above.
        "Table entry (unresolved)",
        "rerun://example.rerun.io/entry/0011223344556677889900aabbccddee",
    ),
    ("Catalog", "rerun://example.rerun.io/catalog"),
    ("Proxy", "rerun://example.rerun.io/proxy"),
    (
        "Folder",
        "rerun://example.rerun.io/folder/perception.detection",
    ),
    ("Intra-recording selection", "recording://camera/points"),
    ("Local file (file:// URL)", "file:///recordings/data.rrd"),
    ("Remote file", "https://example.com/recordings/data.rrd"),
    (
        "Web-viewer share link",
        "https://rerun.io/viewer?url=https://example.com/inner.rrd",
    ),
    ("Non-redap URL (plain link)", "https://rerun.io/"),
];

fn lookup() -> Arc<UrlNameLookup> {
    let origin: re_uri::Origin = "rerun://example.rerun.io".parse().expect("valid origin");

    let mut lookup = UrlNameLookup::default();
    lookup.insert(
        (
            origin.clone(),
            DATASET_TUID.parse::<EntryId>().expect("valid entry id"),
        ),
        ResolvedEntry {
            name: EntryName::new("my-dataset").expect("valid entry name"),
            kind: LinkKind::Dataset,
        },
    );
    lookup.insert(
        (
            origin.clone(),
            DATASET_ENTRY_TUID
                .parse::<EntryId>()
                .expect("valid entry id"),
        ),
        ResolvedEntry {
            name: EntryName::new("my-dataset-entry").expect("valid entry name"),
            kind: LinkKind::Dataset,
        },
    );
    lookup.insert(
        (
            origin,
            TABLE_ENTRY_TUID.parse::<EntryId>().expect("valid entry id"),
        ),
        ResolvedEntry {
            name: EntryName::new("my-table").expect("valid entry name"),
            kind: LinkKind::Table,
        },
    );
    Arc::new(lookup)
}

#[test]
fn link_buttons_match_snapshot() {
    let mut snapshot_results = egui_kittest::SnapshotResults::new();

    for (theme, suffix) in [(egui::Theme::Dark, "dark"), (egui::Theme::Light, "light")] {
        let lookup = lookup();

        let mut harness =
            re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, [480.0, 360.0])
                .with_theme(theme)
                .build_ui(move |ui| {
                    re_ui::apply_style_and_install_loaders(ui.ctx());
                    UrlDecorator::set(ui.ctx(), make_url_decorator(lookup.clone(), theme));

                    egui::Grid::new("link_buttons")
                        .num_columns(2)
                        .show(ui, |ui| {
                            for (description, url) in URLS {
                                ui.label(*description);
                                UiLayout::List.data_label(
                                    ui,
                                    SyntaxHighlightedBuilder::new().with_string_value(url),
                                );
                                ui.end_row();
                            }
                        });
                });

        harness.fit_contents();
        snapshot_results.add(harness.try_snapshot(format!("link_buttons_{suffix}")));
    }
}

/// Hovering a button reveals the copy button: the button content yields space for it by truncating,
/// so the overall layout doesn't shift, and the button frame is painted behind the content only —
/// not behind the copy button.
#[test]
fn link_button_hovered_reveals_copy_button() {
    let lookup = lookup();
    let url = "rerun://example.rerun.io/dataset/1830B33B45B963E7774455beb91701ae?segment_id=segment-abc-12";

    let mut harness = re_ui::testing::new_harness(re_ui::testing::TestOptions::Gui, [320.0, 80.0])
        .build_ui(move |ui| {
            re_ui::apply_style_and_install_loaders(ui.ctx());
            UrlDecorator::set(
                ui.ctx(),
                make_url_decorator(lookup.clone(), ui.ctx().theme()),
            );

            UiLayout::List.data_label(ui, SyntaxHighlightedBuilder::new().with_string_value(url));
        });

    harness.hover_at(egui::pos2(30.0, 10.0));

    // Run twice to ensure the tooltip is shown.
    harness.run();
    harness.run();

    harness.snapshot("link_button_hovered");
}
