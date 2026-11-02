//! Picking which segments of a dataset load an asset.

use std::sync::Arc;

use crossbeam::channel::Receiver;
use egui::AtomExt as _;

use re_async::{AsyncRuntimeHandle, UiWaker};
use re_catalog_client::{AssetCoverage, CatalogClient};
use re_format::{format_plural_s, format_uint};
use re_log_types::EntryId;
use re_log_types::external::re_types_core::SegmentId;
use re_protos::cloud::v1alpha1::ext::AssetMode;
use re_quota_channel::send_crossbeam;
use re_redap_client::{ApiError, ConnectionRegistryHandle};
use re_ui::{
    ButtonExt as _, DesignTokens, ServerValue, UiExt as _,
    modal::{ModalHandler, ModalWrapper},
};

use crate::context::Context;
use crate::entry_meta::{DatasetSegments, StoredCoverage};
use crate::servers::{Command, PILL_MARGIN};

/// How wide a modal holding the coverage cards is, so a heading, its count and the search hint
/// under them all fit on their line.
pub const COVERAGE_MODAL_WIDTH: f32 = 556.0;

pub const ASSETS_DOCS_URL: &str =
    "https://rerun.io/docs/concepts/query-and-transform/catalog-object-model#assets";

pub const COVERAGE_NOT_STORED: &str =
    "This server does not support editing coverage. The asset loads in every segment.";

/// How tall the scrolling body of a modal is at most, leaving room for the footer under it.
///
/// The footer height is the one [`remember_footer_height`] measured the frame before.
pub fn modal_body_max_height(ui: &egui::Ui, footer_id: egui::Id) -> f32 {
    let footer_height = ui
        .data(|data| data.get_temp::<f32>(footer_id))
        .unwrap_or_default();

    (ui.available_height() - footer_height).max(0.0)
}

/// Stores the height of the footer shown in `footer_rect`, for [`modal_body_max_height`] to read
/// the next frame.
pub fn remember_footer_height(ui: &egui::Ui, footer_id: egui::Id, footer_rect: egui::Rect) {
    let height = footer_rect.height() + ui.spacing().item_spacing.y;
    let previous = ui.data_mut(|data| data.get_temp::<f32>(footer_id));

    if previous != Some(height) {
        ui.data_mut(|data| data.insert_temp(footer_id, height));
        ui.ctx().request_repaint();
    }
}

/// The dataset an asset is registered into.
#[derive(Clone, Debug)]
pub struct AssetTarget {
    pub origin: re_uri::Origin,

    pub dataset_id: EntryId,
}

#[derive(Default)]
struct ModeCoverage {
    segments: Vec<SegmentId>,
    search: String,
}

/// The coverage of one asset, as the modals edit it.
///
/// Each mode keeps its own segment list, so switching mode and back keeps what was listed. Only
/// the list of the picked mode is stored.
#[derive(Default)]
pub struct CoverageEditor {
    mode: AssetMode,
    opt_in: ModeCoverage,
    opt_out: ModeCoverage,
}

impl CoverageEditor {
    /// An editor listing a stored coverage under its own mode.
    fn new(coverage: AssetCoverage) -> Self {
        let AssetCoverage { mode, segments } = coverage;

        let mut editor = Self {
            mode,
            ..Default::default()
        };
        editor.mode_mut(mode).segments = segments;

        editor
    }

    /// What the editor stores: the picked mode, and the segments listed under it.
    pub fn coverage(&self) -> AssetCoverage {
        AssetCoverage {
            mode: self.mode,
            segments: self.mode_coverage(self.mode).segments.clone(),
        }
    }

    fn mode_coverage(&self, mode: AssetMode) -> &ModeCoverage {
        match mode {
            AssetMode::OptIn => &self.opt_in,
            AssetMode::OptOut => &self.opt_out,
        }
    }

    fn mode_mut(&mut self, mode: AssetMode) -> &mut ModeCoverage {
        match mode {
            AssetMode::OptIn => &mut self.opt_in,
            AssetMode::OptOut => &mut self.opt_out,
        }
    }
}

pub struct AssetEditState {
    pub target: AssetTarget,
    pub client: CatalogClient,

    pub coverage: CoverageEditor,
}

/// Collapsed, the list shows up to this many segments.
const COLLAPSED_SHOWN_COUNT: usize = 3;

fn segment_list(
    mode: AssetMode,
    ui: &mut egui::Ui,
    segments: &mut Vec<SegmentId>,
    max_list_height: f32,
) {
    /// The expanded list scrolls once it grows past this.
    const MAX_LIST_HEIGHT: f32 = 176.0;

    /// The expanded list shows at least this many rows, and the modal scrolls when it has no room
    /// for them.
    const MIN_LIST_ROWS: f32 = 3.0;

    let expanded_id = ui.make_persistent_id(("asset_coverage_expanded", mode));

    let mut expanded = ui
        .memory(|mem| mem.data.get_temp::<bool>(expanded_id))
        .unwrap_or(false);

    let old_expanded = expanded;

    // A list short enough to show in full collapsed has nothing to expand.
    if expanded && segments.len() > COLLAPSED_SHOWN_COUNT {
        let min_list_height =
            MIN_LIST_ROWS * DesignTokens::list_item_height(re_ui::ListItemDensity::Spacious);
        egui::ScrollArea::vertical()
            .id_salt(("asset_coverage_segments", mode))
            .max_height(max_list_height.clamp(min_list_height, MAX_LIST_HEIGHT))
            .min_scrolled_height(0.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let shown = segments.len();
                segment_pills(ui, &mut expanded, segments, shown);
            });

        ui.horizontal(|ui| {
            let link_color = ui.tokens().card_link_color;
            if ui
                .link(egui::RichText::new("Show Less").color(link_color))
                .clicked()
            {
                expanded = false;
            }
            if ui
                .link(egui::RichText::new("Clear All").color(ui.visuals().weak_text_color()))
                .clicked()
            {
                segments.clear();
                expanded = false;
            }
        });
    } else {
        let shown = segments.len().min(COLLAPSED_SHOWN_COUNT);
        segment_pills(ui, &mut expanded, segments, shown);
    }

    if old_expanded != expanded {
        ui.memory_mut(|mem| mem.data.insert_temp(expanded_id, expanded));
    }
}

/// [`PILL_MARGIN`] with more room above and below, so the remove button doesn't touch the outline,
/// and less on the right, since the button's hover fill already pads the icon.
const SEGMENT_PILL_MARGIN: egui::Margin = egui::Margin {
    top: PILL_MARGIN.top + 2,
    bottom: PILL_MARGIN.bottom + 2,
    right: 4,
    ..PILL_MARGIN
};

/// The first `shown` segments as pills, followed by a link that expands the list when segments
/// are left out.
fn segment_pills(
    ui: &mut egui::Ui,
    expanded: &mut bool,
    segments: &mut Vec<SegmentId>,
    shown: usize,
) {
    let max_width = ui.available_width();
    ui.horizontal_wrapped(|ui| {
        /// The gap between a pill's text and its remove button.
        const TEXT_BUTTON_GAP: f32 = 6.0;

        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        // The remove button is the bare icon, so the pill's margins alone set its spacing.
        ui.spacing_mut().button_padding = egui::Vec2::ZERO;
        let remove_id = egui::IdSalt::new("remove_segment");
        let pill_frame = egui::Frame::new()
            .fill(ui.tokens().panel_bg_color)
            .corner_radius(6)
            .stroke(ui.tokens().card_stroke)
            .inner_margin(SEGMENT_PILL_MARGIN);
        let mut i = 0;
        while i < shown {
            let Some(segment) = segments.get(i) else {
                break;
            };

            let response = egui::WidgetAtom::new((
                egui::RichText::new(segment.as_str())
                    .monospace()
                    .atom_shrink(true),
                egui::Atom::custom(remove_id, ui.tokens().small_icon_size),
            ))
            .frame(pill_frame)
            .gap(TEXT_BUTTON_GAP)
            .min_size(egui::vec2(0.0, ui.spacing().interact_size.y))
            .max_width(max_width)
            .wrap_mode(egui::TextWrapMode::Truncate)
            .show(ui);
            let remove_rect = response.rect(remove_id);
            response.response.on_hover_text(segment.as_str());

            // Sized to the icon's rect, so the theme's minimum button height doesn't grow it out of
            // the pill.
            if let Some(rect) = remove_rect
                && ui
                    .place(
                        rect,
                        ui.small_icon_button_widget(&re_ui::icons::CLOSE_SMALL, "remove")
                            .size(re_ui::Size::custom(rect.height())),
                    )
                    .clicked()
            {
                segments.remove(i);
            } else {
                i += 1;
            }
        }

        if let Some(extra) = segments.len().checked_sub(i)
            && extra > 0
            && ui
                .link(
                    egui::RichText::new(format!("+{} More", format_uint(extra)))
                        .color(ui.tokens().card_link_color),
                )
                .clicked()
        {
            *expanded = true;
        }
    });
}

/// Whether `text` contains `pattern`, with ASCII letters matching either case.
fn contains_ignore_ascii_case(text: &str, pattern: &str) -> bool {
    pattern.is_empty()
        || text
            .as_bytes()
            .windows(pattern.len())
            .any(|window| window.eq_ignore_ascii_case(pattern.as_bytes()))
}

/// The first segments of the dataset the search text matches and that aren't listed yet.
fn segment_matches<'a>(search: &str, ids: &'a [SegmentId], listed: &[SegmentId]) -> Vec<&'a str> {
    /// Only offer up to this many matches at a time.
    const MAX_MATCHES: usize = 50;

    let listed: std::collections::HashSet<&SegmentId> = listed.iter().collect();

    ids.iter()
        .filter(|id| contains_ignore_ascii_case(id.as_str(), search) && !listed.contains(id))
        .map(SegmentId::as_str)
        .take(MAX_MATCHES)
        .collect()
}

/// The segments the search text matches, as suggestions that add the segment to the list.
fn segment_suggestions(
    search: &str,
    segments: &[SegmentId],
    dataset_segments: &DatasetSegments,
) -> Vec<egui::Suggestion> {
    let Some(ids) = dataset_segments.ids.get() else {
        return Vec::new();
    };

    if search.is_empty() {
        return Vec::new();
    }

    segment_matches(search, ids.ids(), segments)
        .into_iter()
        .map(|segment| {
            egui::Suggestion::new(segment).content(egui::RichText::new(segment).monospace())
        })
        .collect()
}

/// The line under the search field while the search text offers no segments.
fn search_status_ui(
    ui: &mut egui::Ui,
    search: &str,
    no_matches: bool,
    dataset_segments: &DatasetSegments,
) {
    if search.is_empty() {
        return;
    }

    if dataset_segments.ids.get().is_some() {
        if no_matches {
            ui.weak("No segment matches");
        }
    } else if let Some(err) = dataset_segments.ids.get_err() {
        ui.error_label(err.to_string());
    } else {
        ui.horizontal(|ui| {
            ui.inline_loading_indicator("loading the segments to search");
            ui.weak("Waiting for the dataset's segments");
        });
    }
}

/// How many segments of the dataset `listed` leaves out, or `None` while the count is unknown.
fn unlisted_segment_count(
    dataset_segments: &DatasetSegments,
    listed: &[SegmentId],
) -> Option<usize> {
    // A listed segment can be one the dataset no longer has. Until the segment ids are loaded,
    // every listed segment counts. After that, only the ones the dataset has count.
    let listed_in_dataset = match dataset_segments.ids.get() {
        Some(ids) if !ids.is_empty() => ids.count_held(listed),
        _ => listed.len(),
    };

    dataset_segments
        .count
        .map(|count| count.saturating_sub(listed_in_dataset))
}

/// What the card of one [`AssetMode`] reads.
struct ModeLabels {
    /// What the mode does with the segments, leading into the segment list.
    heading: &'static str,

    /// The line shown while the segment list is empty, if any.
    empty: Option<&'static str>,

    /// What listing a segment does to it, as in "search … to exclude".
    verb: &'static str,

    /// What a listed segment is, as in "6 excluded".
    listed: &'static str,
}

impl ModeLabels {
    fn new(mode: AssetMode) -> Self {
        match mode {
            AssetMode::OptOut => Self {
                heading: "All segments except these:",
                empty: None,
                verb: "exclude",
                listed: "excluded",
            },
            AssetMode::OptIn => Self {
                heading: "Only these segments:",
                empty: Some("No segments selected yet"),
                verb: "select",
                listed: "selected",
            },
        }
    }

    /// The hint of the search field, with the number of segments not listed yet once it is known.
    fn search_hint(&self, segment_count: Option<usize>) -> String {
        let verb = self.verb;

        match segment_count {
            Some(count) => format!("Search {} to {verb}…", format_plural_s(count, "segment")),
            None => format!("Search the segments to {verb}…"),
        }
    }
}

impl CoverageEditor {
    /// One card per [`AssetMode`], the selected one holding the segment list that applies.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        dataset_segments: &DatasetSegments,
        footer_height: f32,
    ) {
        // Each card reserves space for its heading, list buttons, search field and padding.
        const CARD_CONTROLS_HEIGHT: f32 = 140.0;
        let max_list_height =
            ((ui.available_height() - footer_height - 5.0) / 2.0 - CARD_CONTROLS_HEIGHT).max(0.0);
        self.show_mode(AssetMode::OptOut, ui, dataset_segments, max_list_height);
        ui.add_space(5.0);
        self.show_mode(AssetMode::OptIn, ui, dataset_segments, max_list_height);
    }

    pub fn summary_ui(&self, ui: &mut egui::Ui, dataset_segments: &DatasetSegments) {
        let segments = &self.mode_coverage(self.mode).segments;
        if let Some(ids) = dataset_segments.ids.get() {
            let applies = ids.count_covered(self.mode, segments);
            // An asset that applies to no segment never loads, which is likely a mistake.
            let color = (applies == 0).then(|| ui.visuals().warn_fg_color);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let text = |text: String| {
                    let text = egui::RichText::new(text);
                    match color {
                        Some(color) => text.color(color),
                        None => text,
                    }
                };
                ui.label(text("Applies to ".to_owned()));
                ui.label(
                    text(format!(
                        "{} of {} segments",
                        format_uint(applies),
                        format_uint(ids.len())
                    ))
                    .strong(),
                );
            });
        } else {
            ui.weak(format!(
                "{} {}",
                format_uint(segments.len()),
                ModeLabels::new(self.mode).listed
            ));
        }
    }

    fn show_mode(
        &mut self,
        mode: AssetMode,
        ui: &mut egui::Ui,
        dataset_segments: &DatasetSegments,
        max_list_height: f32,
    ) {
        /// Space between the heading of a card, its segment list and its search field.
        const CARD_LINE_SPACE: f32 = 8.0;

        let selected = self.mode == mode;
        let tokens = ui.tokens();

        let mut frame = card_frame(tokens)
            .inner_margin(egui::Margin::symmetric(14, 13))
            .corner_radius(11);
        if selected {
            frame = frame
                .fill(tokens.card_selected_fill)
                .stroke(tokens.card_selected_stroke);
        }

        let labels = ModeLabels::new(mode);

        let mut prepared = frame.begin(ui);
        {
            let ui = &mut prepared.content_ui;
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = CARD_LINE_SPACE;

            ui.re_radio_value(
                &mut self.mode,
                mode,
                egui::RichText::new(labels.heading).strong(),
            );

            let ModeCoverage { segments, search } = self.mode_mut(mode);

            // Indented to where the radio button's label starts, so the list lines up with the
            // heading.
            let label_indent = ui.spacing().icon_width + ui.spacing().icon_spacing;
            egui::Frame::new()
                .inner_margin(egui::Margin {
                    left: label_indent.round() as i8,
                    ..Default::default()
                })
                .show(ui, |ui| {
                    if !segments.is_empty() {
                        ui.scope(|ui| {
                            // The list of the mode that isn't selected doesn't apply, so it is
                            // drawn faded.
                            if !selected {
                                ui.multiply_opacity(0.45);
                            }
                            segment_list(mode, ui, segments, max_list_height);
                        });
                    } else if let Some(empty) = labels.empty {
                        ui.label(egui::RichText::new(empty).size(11.5));
                    }

                    // Only the search field of the selected mode is enabled. Clicking the other
                    // field selects its mode through the click on the card.
                    if !selected {
                        ui.disable();
                    }

                    // The padding around the field is on a separate frame. The field keeps its
                    // own margin, which keeps its size the same on hover and focus.
                    let hint =
                        labels.search_hint(unlisted_segment_count(dataset_segments, segments));
                    // The popup skips the suggestions after Escape, which isn't a missing match.
                    let mut no_matches = false;
                    let completion = egui::Frame::new()
                        .inner_margin(egui::Margin {
                            left: 0,
                            right: 7,
                            top: 0,
                            bottom: 6,
                        })
                        .show(ui, |ui| {
                            egui::CompletionPopup::new(
                                ui.make_persistent_id(("asset_coverage_search", mode)),
                            )
                            .align(egui::RectAlign::BOTTOM_START)
                            // The whole search text is matched, spaces included.
                            .word_boundary(|_| false)
                            .show(
                                ui,
                                egui::TextEdit::singleline(search)
                                    .background_color(if selected {
                                        ui.tokens().card_text_edit_bg_color
                                    } else {
                                        ui.visuals().text_edit_bg_color()
                                    })
                                    .desired_width(ui.available_width())
                                    .hint_text({
                                        let hint = egui::RichText::new(hint);
                                        // A disabled field fades everything it paints, so the
                                        // hint uses the default text color.
                                        if selected {
                                            hint
                                        } else {
                                            hint.color(ui.tokens().text_default)
                                        }
                                    }),
                                |query| {
                                    let suggestions =
                                        segment_suggestions(query.text, segments, dataset_segments);
                                    no_matches = suggestions.is_empty();
                                    suggestions
                                },
                            )
                        })
                        .inner;

                    if let Some(accepted) = completion.accepted {
                        segments.push(SegmentId::from(accepted.insert));
                        search.clear();
                    } else if selected {
                        search_status_ui(ui, search, no_matches, dataset_segments);
                    }
                });
        }

        // Only a mode that isn't selected reacts to the pointer, since clicking the selected one
        // changes nothing. A disabled editor reacts to nothing.
        let reacts_to_pointer = !selected && ui.is_enabled();
        let card_rect = prepared.content_ui.min_rect() + prepared.frame.inner_margin;
        if reacts_to_pointer && ui.rect_contains_pointer(card_rect) {
            prepared.frame = prepared
                .frame
                .fill(tokens.card_hover_fill)
                .stroke(tokens.card_hover_stroke);
        }

        let response = prepared.end(ui);

        if reacts_to_pointer
            && (response.clicked()
                || (ui.rect_contains_pointer(response.rect)
                    && ui.input(|i| i.pointer.primary_pressed())))
        {
            self.mode = mode;
        }
    }
}

/// Modal for picking which segments of a dataset load an asset.
#[derive(Default)]
pub struct AssetCoverageModal {
    modal: ModalHandler,

    state: Option<State>,
}

/// What the modal works on while it is open.
struct State {
    edit_state: AssetEditState,

    /// The asset whose coverage the modal edits.
    asset_id: SegmentId,

    /// The coverage and revision used to initialize the editor.
    stored: Option<Arc<StoredCoverage>>,

    /// The write the server is working on, and the answer to it.
    saving: Option<Receiver<Result<u64, ApiError>>>,

    /// Why the last write failed.
    error: Option<String>,
}

impl State {
    /// Starts writing the edited coverage to the server.
    fn save(&mut self, ctx: &Context<'_>, waker: UiWaker) {
        let Some(revision) = self.stored.as_ref().map(|stored| stored.revision) else {
            return;
        };
        let AssetTarget { origin, dataset_id } = self.edit_state.target.clone();
        let client = self.edit_state.client.clone();
        let coverage = self.edit_state.coverage.coverage();
        let asset_id = self.asset_id.clone();
        let command_sender = ctx.command_sender.clone();
        let (tx, rx) = re_quota_channel::create_crossbeam_channel(1);

        self.error = None;
        self.saving = Some(rx);

        // The segments the asset covers change, so they are read again. This runs
        // here rather than where the answer is shown, so closing the modal on a
        // write the server is still working on leaves it as it lands.
        self.edit_state.client.runtime().spawn_future(async move {
            let result = client
                .set_asset_coverage(dataset_id, asset_id.clone(), &coverage, Some(revision))
                .await;

            match &result {
                Ok(_revision) => {
                    send_crossbeam(
                        &command_sender,
                        Command::AssetsChanged {
                            origin,
                            entry_id: dataset_id,
                            reload_segments: true,
                            unregistered_asset: None,
                        },
                    )
                    .ok();
                }

                Err(err) => {
                    re_log::error!("Failed writing the asset's coverage: {err}\nAsset: {asset_id}");
                }
            }

            send_crossbeam(&tx, result).ok();
            waker.wake();
        });
    }
}

impl AssetCoverageModal {
    pub fn open(
        &mut self,
        target: AssetTarget,
        asset_id: SegmentId,
        connection_registry: &ConnectionRegistryHandle,
        runtime: AsyncRuntimeHandle,
    ) {
        self.state = Some(State {
            edit_state: AssetEditState {
                client: CatalogClient::new(
                    connection_registry.connection_handle(target.origin.clone()),
                    runtime,
                ),
                target,
                coverage: Default::default(),
            },
            asset_id,
            stored: Default::default(),
            saving: None,
            error: None,
        });

        self.modal.open();
    }

    /// The dataset the modal edits the coverage in, while it is open.
    pub fn target(&self) -> Option<&AssetTarget> {
        self.state.as_ref().map(|state| &state.edit_state.target)
    }

    pub fn asset_id(&self) -> Option<&SegmentId> {
        self.state.as_ref().map(|state| &state.asset_id)
    }

    pub fn ui(
        &mut self,
        ctx: &Context<'_>,
        ui: &egui::Ui,
        dataset_segments: &DatasetSegments,
        stored: Option<&ServerValue<Arc<StoredCoverage>, ApiError>>,
    ) {
        let Some(state) = &mut self.state else {
            return;
        };

        let asset_id = state.asset_id.clone();

        // The editor starts on the coverage the asset has, and is the user's from then on.
        if state.stored.is_none()
            && let Some(stored) = stored.and_then(ServerValue::get)
        {
            state.edit_state.coverage = CoverageEditor::new(stored.coverage.clone());
            state.stored = Some(stored.clone());
        }

        self.modal.ui(
            ui.ctx(),
            || {
                ModalWrapper::new(&format!("{asset_id} coverage"))
                    .max_width(COVERAGE_MODAL_WIDTH)
                    .max_height(f32::INFINITY)
            },
            |ui| {
                if let Some(rx) = &state.saving
                    && let Ok(result) = rx.try_recv()
                {
                    state.saving = None;

                    match result {
                        // The dataset reads its segments again on its own, see where the write is
                        // started.
                        Ok(_revision) => ui.close(),

                        Err(err) => state.error = Some(err.to_string()),
                    }
                }

                let is_editable = state
                    .stored
                    .as_ref()
                    .is_some_and(|stored| stored.is_editable);
                let footer_id = ui.make_persistent_id("asset_coverage_footer");

                egui::ScrollArea::vertical()
                    .id_salt("asset_coverage_body")
                    .max_height(modal_body_max_height(ui, footer_id))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.add_space(4.0);
                        intro_ui(ui, &asset_id);
                        ui.add_space(8.0);

                        if is_editable {
                            state.edit_state.coverage.ui(ui, dataset_segments, 0.0);
                        } else if state.stored.is_some() {
                            ui.weak(COVERAGE_NOT_STORED);
                        } else if let Some(err) = stored.and_then(ServerValue::get_err) {
                            ui.error_label(err.to_string());
                        } else {
                            ui.loading_indicator("waiting for the asset's coverage");
                        }

                        if let Some(err) = &state.error {
                            ui.add_space(4.0);
                            ui.error_label(err);
                        }
                    });

                let saving = state.saving.is_some();
                let footer = ui.scope(|ui| {
                    ui.add_space(16.0);
                    egui::Sides::new().shrink_left().truncate().show(
                        ui,
                        |ui| {
                            if saving {
                                ui.inline_loading_indicator("writing the coverage");
                                ui.weak("Waiting for the server to take it");
                            } else {
                                state.edit_state.coverage.summary_ui(ui, dataset_segments);
                            }
                        },
                        |ui| {
                            let can_save = is_editable && !saving;
                            let save = egui::Button::new("Save coverage").blue().size_small();
                            let save_clicked = ui.add_enabled(can_save, save).clicked();

                            if ui.add(egui::Button::new("Cancel").size_small()).clicked() {
                                ui.close();
                            }

                            can_save && save_clicked
                        },
                    )
                });

                if footer.inner.1 {
                    state.save(ctx, ui.ui_waker());
                }

                remember_footer_height(ui, footer_id, footer.response.rect);
            },
        );

        if !self.modal.is_open() {
            self.state = None;
        }
    }
}

/// What the modal does, followed by a link to the asset docs.
fn intro_ui(ui: &mut egui::Ui, asset_id: &SegmentId) {
    ui.horizontal_wrapped(|ui| {
        ui.label(format!(
            "Choose which segments load {asset_id} asset in the Viewer. For large selections set \
             coverage from the SDK."
        ));
        assets_docs_link(ui);
    });
}

/// A "Docs" link to the asset docs, with an icon that marks it as an external link.
pub fn assets_docs_link(ui: &mut egui::Ui) {
    /// Space between the word "Docs" and the icon that marks it as a link.
    const LINK_ICON_GAP: f32 = 4.0;

    let link_color = ui.tokens().card_link_color;
    if ui
        .add(
            egui::WidgetAtom::new((
                egui::RichText::new("Docs").color(link_color),
                re_ui::icons::EXTERNAL_LINK
                    .as_image()
                    .tint(link_color)
                    .atom_max_height_font_size(ui),
            ))
            .gap(LINK_ICON_GAP)
            .sense(egui::Sense::click()),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        ui.open_url(egui::OpenUrl::new_tab(ASSETS_DOCS_URL));
    }
}

pub fn card_frame(tokens: &DesignTokens) -> egui::Frame {
    let inner_margin = egui::Margin::same(tokens.table_grid_view_card_inner_margin as i8);

    egui::Frame::new()
        .inner_margin(inner_margin)
        .fill(tokens.card_fill)
        .stroke(tokens.card_stroke)
        .corner_radius(tokens.table_grid_view_card_corner_radius)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use egui::accesskit::Role;
    use egui_kittest::kittest::Queryable as _;
    use re_ui::ServerValue;

    use super::*;
    use crate::entry_meta::SegmentIds;

    /// With nothing listed, each card says what its own list would do, and the card of the
    /// selected mode is the one that reads as picked.
    #[test]
    fn an_empty_coverage_shows_what_each_mode_does() {
        run_test(CoverageEditor::default(), "asset_coverage__empty");
    }

    /// The listed segments show as pills on their own card, above its search field.
    #[test]
    fn listed_segments_show_as_pills() {
        let coverage = CoverageEditor {
            mode: AssetMode::OptIn,
            opt_in: ModeCoverage {
                segments: vec![
                    SegmentId::from("episode_0001"),
                    SegmentId::from("episode_0002"),
                ],
                ..Default::default()
            },
            ..Default::default()
        };

        run_test(coverage, "asset_coverage__opt_in_with_segments");
    }

    /// The segments the search text matches are the ones offered, minus the ones already listed,
    /// and only the first matches in dataset order.
    #[test]
    fn the_search_offers_the_segments_it_matches() {
        let ids: Vec<SegmentId> = (0..100)
            .map(|index| SegmentId::from(format!("episode_{index:04}")))
            .collect();

        let matches = segment_matches("EPISODE_00", &ids, &[]);
        assert_eq!(matches.len(), 50);
        assert_eq!(matches.first(), Some(&"episode_0000"));
        assert_eq!(matches.last(), Some(&"episode_0049"));

        let listed = vec![SegmentId::from("episode_0001")];
        assert!(segment_matches("0001", &ids, &listed).is_empty());

        assert!(segment_matches("nothing", &ids, &[]).is_empty());
    }

    /// Clicking a segment the search offers adds it to the list of the selected mode and clears
    /// the search.
    #[test]
    fn clicking_a_search_match_lists_it() {
        let segment_ids = (0..4)
            .map(|index| SegmentId::from(format!("episode_{index:04}")))
            .collect();
        let mut harness = harness_with_segment_ids(CoverageEditor::default(), 600.0, segment_ids);

        let search = harness.query_all_by_role(Role::TextInput).next().unwrap();
        search.focus();
        search.type_text("0002");
        harness.run();

        harness.get_by_label("episode_0002").click();
        harness.run();

        let coverage = harness.state().coverage();
        assert_eq!(coverage.mode, AssetMode::OptOut);
        assert_eq!(coverage.segments, [SegmentId::from("episode_0002")]);
        assert!(harness.state().opt_out.search.is_empty());
    }

    /// A stored coverage lands on the card of its own mode, and comes back out the same way.
    /// Switching to the other mode stores that mode's own list, and switching back gives the
    /// stored list again.
    #[test]
    fn a_stored_coverage_round_trips_through_the_editor() {
        for (mode, other_mode) in [
            (AssetMode::OptIn, AssetMode::OptOut),
            (AssetMode::OptOut, AssetMode::OptIn),
        ] {
            let coverage = AssetCoverage {
                mode,
                segments: vec![
                    SegmentId::from("episode_0001"),
                    SegmentId::from("episode_0002"),
                ],
            };

            let mut editor = CoverageEditor::new(coverage.clone());
            assert_eq!(editor.coverage(), coverage);

            editor.mode = other_mode;
            assert_eq!(
                editor.coverage(),
                AssetCoverage {
                    mode: other_mode,
                    segments: vec![],
                }
            );

            editor.mode = mode;
            assert_eq!(editor.coverage(), coverage);
        }
    }

    /// With more segments listed than the card shows, the first few show as pills followed by a
    /// link that expands the list. Expanded, every listed segment shows as a pill in a scroll
    /// area.
    #[test]
    fn many_listed_segments_expand_into_a_scroll_area() {
        let coverage = CoverageEditor {
            mode: AssetMode::OptOut,
            opt_out: ModeCoverage {
                segments: (1..=6)
                    .map(|index| SegmentId::from(format!("episode_{index:04}")))
                    .collect(),
                ..Default::default()
            },
            ..Default::default()
        };

        let mut harness = harness(coverage, 600.0);
        harness.snapshot("asset_coverage__opt_out_many_segments");

        harness.get_by_label("+3 More").click_accesskit();
        harness.run();
        harness.snapshot("asset_coverage__opt_out_expanded");
    }

    /// Clearing an expanded list removes every listed segment.
    #[test]
    fn clear_all_empties_the_expanded_list() {
        let coverage = CoverageEditor {
            mode: AssetMode::OptOut,
            opt_out: ModeCoverage {
                segments: (1..=6)
                    .map(|index| SegmentId::from(format!("episode_{index:04}")))
                    .collect(),
                ..Default::default()
            },
            ..Default::default()
        };

        let mut harness = harness(coverage, 600.0);
        harness.get_by_label("+3 More").click_accesskit();
        harness.run();
        harness.get_by_label("Clear All").click_accesskit();
        harness.run();

        assert!(harness.state().coverage().segments.is_empty());
    }

    /// In the expanded list, a segment name too long for its pill is truncated before the remove
    /// button, and the pill keeps the width of the card.
    #[test]
    fn long_segment_names_truncate_in_the_expanded_list() {
        let coverage = CoverageEditor {
            mode: AssetMode::OptIn,
            opt_in: ModeCoverage {
                segments: (1..=6)
                    .map(|index| {
                        SegmentId::from(format!(
                            "place_and_organize_the_fake_fruits_in_the_fruit_bowl__0499e571-6058-465f-9c41-{index:012}"
                        ))
                    })
                    .collect(),
                ..Default::default()
            },
            ..Default::default()
        };

        let mut harness = harness(coverage, 600.0);
        harness.get_by_label("+3 More").click_accesskit();
        harness.run();
        harness.snapshot("asset_coverage__opt_in_expanded_long_names");
    }

    fn run_test(coverage: CoverageEditor, snapshot_name: &str) {
        harness(coverage, 320.0).snapshot(snapshot_name);
    }

    fn harness(
        coverage: CoverageEditor,
        height: f32,
    ) -> egui_kittest::Harness<'static, CoverageEditor> {
        harness_with_segment_ids(coverage, height, Vec::new())
    }

    fn harness_with_segment_ids(
        coverage: CoverageEditor,
        height: f32,
        segment_ids: Vec<SegmentId>,
    ) -> egui_kittest::Harness<'static, CoverageEditor> {
        /// How many segments the dataset holds, as the search fields name it.
        const SEGMENT_COUNT: usize = 2048;

        let dataset_segments = DatasetSegments {
            count: Some(SEGMENT_COUNT),
            ids: ServerValue::Completed(Arc::new(SegmentIds::new(segment_ids))),
        };

        let mut harness = re_ui::testing::new_harness(
            re_ui::testing::TestOptions::Gui,
            egui::Vec2::new(COVERAGE_MODAL_WIDTH, height),
        )
        .build_ui_state(
            move |ui, coverage: &mut CoverageEditor| {
                re_ui::apply_style_and_install_loaders(ui.ctx());

                coverage.ui(ui, &dataset_segments, 0.0);
            },
            coverage,
        );

        harness.run();
        harness
    }
}
