use egui::{IntoAtoms as _, NumExt as _, Ui};
use ehttp::{Request, fetch};
use itertools::Itertools as _;
use poll_promise::Promise;

use crate::ui::CloudState;
use crate::ui::welcome_screen::entrance_animation::{EntranceAnimation, EntranceAnimationItem};
use crate::ui::welcome_screen::intro_section::intro_section;
use crate::ui::welcome_screen::welcome_section::welcome_section_ui;
use re_ui::{DesignTokens, Icon, UiExt as _};
use re_viewer_context::AppContext;

const TAG_ICON: Icon = Icon::new(
    "welcome_tag.svg",
    include_bytes!("../../../data/welcome_tag.svg"),
);

#[derive(Debug, serde::Deserialize)]
struct ExampleThumbnail {
    url: String,
    width: u32,
    height: u32,
}

#[derive(Debug, serde::Deserialize)]
struct ExampleDesc {
    /// `snake_case` version of the example name
    name: String,

    /// human-readable version of the example name
    title: String,

    tags: Vec<String>,

    rrd_url: String,
    thumbnail: ExampleThumbnail,

    /// URL of the source code in GitHub
    source_url: Option<String>,
}

pub(super) const MIN_COLUMN_WIDTH: f32 = 250.0;
const MAX_COLUMN_WIDTH: f32 = 260.0;
const MAX_CONTENT_WIDTH: f32 = 828.0;
const COLUMN_HSPACE: f32 = 16.0;
const AFTER_HEADER_VSPACE: f32 = 90.0;
const AFTER_SEPARATOR_VSPACE: f32 = 62.0;
const TITLE_TO_GRID_VSPACE: f32 = 14.0;
const CARD_ASPECT_RATIO: f32 = 260.0 / 340.0;
const CARD_RADIUS: u8 = 16;
const DESCRIPTION_INNER_MARGIN: egui::Vec2 = egui::vec2(16.0, 10.0);
const TITLE_TO_METADATA_VSPACE: f32 = 4.0;
const METADATA_ITEM_SPACING: egui::Vec2 = egui::vec2(8.0, 4.0);
const METADATA_ATOM_GAP: f32 = 4.0;
const EXAMPLE_HEADING_DELAY: f32 = 0.06;
const EXAMPLE_HEADING_ANIMATION: EntranceAnimationItem = EntranceAnimationItem::new(0.30, 0.20);
const EXAMPLE_CARD_ANIMATION: EntranceAnimationItem = EntranceAnimationItem::new(0.40, 0.055);

/// Loaded example data and the asynchronous file-size lookup displayed in its card.
struct ExampleDescLayout {
    desc: ExampleDesc,

    /// We do an async HEAD request to get the size of the RRD file
    /// so we can show it to the user.
    rrd_byte_size_promise: Promise<Option<u64>>,
}

enum MetadataItem<'a> {
    Tag(&'a str),
    Source(&'a str),
    FileSize(String),
}

impl MetadataItem<'_> {
    fn atom_layout(&self, ui: &Ui, id: egui::Id) -> egui::WidgetAtom<'static> {
        let text = |text: &str| {
            egui::RichText::new(text)
                .text_style(DesignTokens::body())
                .color(ui.tokens().text_default)
        };

        let atoms = match self {
            Self::Tag(tag) => (
                TAG_ICON
                    .as_image()
                    .fit_to_exact_size(egui::Vec2::splat(8.0))
                    .tint(ui.tokens().text_default),
                text(tag),
            )
                .into_atoms(),
            Self::Source(_) => (
                re_ui::icons::GITHUB
                    .as_image()
                    .tint(ui.tokens().text_default),
                text("Source"),
            )
                .into_atoms(),
            Self::FileSize(size) => text(size).into_atoms(),
        };

        let sense = if matches!(self, Self::Source(_)) {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        };

        egui::WidgetAtom::new(atoms)
            .id(id)
            .sense(sense)
            .gap(METADATA_ATOM_GAP)
            .wrap_mode(egui::TextWrapMode::Extend)
    }

    fn size(&self, ui: &Ui, available_width: f32) -> egui::Vec2 {
        let sized = self
            .atom_layout(ui, egui::Id::NULL)
            .measure(ui, egui::vec2(available_width, f32::INFINITY));
        let atom_count = sized.len();
        let mut size = egui::Vec2::ZERO;
        for atom in sized.iter() {
            size.x += atom.size.x;
            size.y = size.y.max(atom.size.y);
        }
        size.x += METADATA_ATOM_GAP * atom_count.saturating_sub(1) as f32;
        size
    }
}

struct PositionedMetadataItem<'a> {
    item: MetadataItem<'a>,
    offset: egui::Vec2,
    size: egui::Vec2,
}

struct MetadataLayout<'a> {
    items: Vec<PositionedMetadataItem<'a>>,
    height: f32,
}

impl ExampleDescLayout {
    fn new(egui_ctx: &egui::Context, desc: ExampleDesc) -> Self {
        Self {
            rrd_byte_size_promise: load_file_size(egui_ctx, desc.rrd_url.clone()),
            desc,
        }
    }
}

type ManifestJson = Vec<ExampleDesc>;
type Manifest = Vec<ExampleDescLayout>;
type ManifestPromise = Promise<Result<Manifest, LoadError>>;

enum LoadError {
    Deserialize(serde_json::Error),
    Fetch(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Deserialize(err) => {
                write!(f, "manifest is invalid, it may be outdated: {err}")
            }
            Self::Fetch(err) => f.write_str(err),
        }
    }
}

fn load_manifest(egui_ctx: &egui::Context, url: String) -> ManifestPromise {
    let (sender, promise) = Promise::new();
    let egui_ctx = egui_ctx.clone(); // So we can wake up the ui thread

    fetch(Request::get(url), move |response| {
        match response {
            Ok(response) => sender.send(
                serde_json::from_slice::<ManifestJson>(&response.bytes)
                    .map(|examples| {
                        examples
                            .into_iter()
                            .map(|example| ExampleDescLayout::new(&egui_ctx, example))
                            .collect()
                    })
                    .map_err(LoadError::Deserialize),
            ),
            Err(err) => sender.send(Err(LoadError::Fetch(err))),
        }
        egui_ctx.request_repaint();
    });

    promise
}

/// Do a HEAD request to get the size of a file.
///
/// In case of an error, it is logged as DEBUG and
/// the promise is resolved to `None`.
fn load_file_size(egui_ctx: &egui::Context, url: String) -> Promise<Option<u64>> {
    let (sender, promise) = Promise::new();
    let egui_ctx = egui_ctx.clone(); // So we can wake up the ui thread

    let request = Request::head(url.clone());

    fetch(request, move |response| {
        match response {
            Ok(response) => {
                if response.ok {
                    let headers = &response.headers;
                    let content_length = headers
                        .get("content-length")
                        .or_else(|| headers.get("x-goog-stored-content-length"))
                        .and_then(|s| s.parse::<u64>().ok());
                    sender.send(content_length);
                } else {
                    re_log::debug!(
                        "Failed to load file size of {url:?}: {} {}",
                        response.status,
                        response.status_text
                    );
                    sender.send(None);
                }
            }
            Err(err) => {
                re_log::debug!(?url, "Failed to load file size: {err}");
                sender.send(None);
            }
        }
        egui_ctx.request_repaint();
    });

    promise
}

pub(super) struct ExampleSection {
    id: egui::Id,
    manifest_url: String,
    examples: Option<ManifestPromise>,
    entrance_animation: EntranceAnimation,
}

fn default_manifest_url() -> String {
    // Sometimes we want the default to point somewhere else, such as when doing nightly builds.
    if let Some(url) = option_env!("DEFAULT_EXAMPLES_MANIFEST_URL") {
        return url.into();
    }

    let build_info = re_build_info::build_info!();

    if build_info.version.is_rc() || build_info.version.is_release() {
        // If this is versioned as a release or rc, always point to the versioned
        // example manifest. This applies even if doing a local source build.
        format!(
            "https://app.rerun.io/version/{version}/examples_manifest.json",
            version = build_info.version,
        )
    } else {
        // We don't build examples on each PR, so we don't have much to point to except for the nightly examples
        // We could point to the main branch, but it's not always finished building, and so doesn't always work.
        "https://app.rerun.io/version/nightly/examples_manifest.json".into()
    }
}

impl Default for ExampleSection {
    fn default() -> Self {
        Self {
            id: egui::Id::unique("example_section"),
            manifest_url: default_manifest_url(),
            examples: None,
            entrance_animation: EntranceAnimation::default(),
        }
    }
}

impl ExampleSection {
    pub fn set_manifest_url(&mut self, egui_ctx: &egui::Context, url: String) {
        if self.manifest_url != url {
            self.manifest_url.clone_from(&url);
            self.examples = Some(load_manifest(egui_ctx, url));
        }
    }

    /// Draw the example section of the welcome screen.
    pub(super) fn ui(&mut self, ui: &mut egui::Ui, ctx: &AppContext<'_>, login_state: &CloudState) {
        re_tracing::profile_function!();

        let now = ui.input(|input| input.time);
        let animations_enabled = ui.style().animation_time > 0.0;
        self.entrance_animation.begin_frame(now, animations_enabled);
        let section_id = self.id;

        let examples = self
            .examples
            .get_or_insert_with(|| load_manifest(ui.ctx(), self.manifest_url.clone()));

        let section_x_range = ui.clip_rect().x_range();
        let visible_width = ui
            .available_rect_before_wrap()
            .intersect(ui.clip_rect())
            .width()
            .at_least(1.0);
        let max_width = visible_width.at_most(MAX_CONTENT_WIDTH);

        // vertical spacing isn't homogeneous so it's handled manually
        let grid_spacing = egui::vec2(COLUMN_HSPACE, COLUMN_HSPACE);
        let column_count = (((max_width + grid_spacing.x) / (MIN_COLUMN_WIDTH + grid_spacing.x))
            .floor() as usize)
            .at_least(1);
        let column_width = ((max_width + grid_spacing.x) / column_count as f32 - grid_spacing.x)
            .floor()
            .min(MAX_COLUMN_WIDTH)
            .min(max_width);

        ui.horizontal(|ui| {
            // this space is added on the left so that the grid is centered
            let centering_hspace = (visible_width - max_width).max(0.0) / 2.0;
            ui.add_space(centering_hspace);

            ui.vertical(|ui| {
                ui.set_width(max_width);

                welcome_section_ui(ui, &mut self.entrance_animation);
                intro_section(ui, ctx, login_state, &mut self.entrance_animation);

                ui.add_space(AFTER_HEADER_VSPACE);
                ui.painter().hline(
                    section_x_range,
                    ui.cursor().top(),
                    ui.tokens().native_frame_stroke,
                );
                ui.add_space(AFTER_SEPARATOR_VSPACE);

                let Some(examples) = examples.ready_mut() else {
                    // Still waiting for example to load
                    ui.loading_indicator("Fetching example list"); // Placeholder for the examples
                    return;
                };

                let examples = match examples {
                    Ok(examples) => examples,
                    Err(err) => {
                        // Examples failed to load.
                        re_log::warn_once!("Failed to load examples: {err}");

                        return;
                    }
                };

                if examples.is_empty() {
                    ui.label("No examples found.");
                    return;
                }

                self.entrance_animation.add_delay(EXAMPLE_HEADING_DELAY);
                self.entrance_animation.show(
                    ui,
                    section_id.with("example_heading"),
                    EXAMPLE_HEADING_ANIMATION,
                    |ui| {
                        ui.add(egui::Label::new(
                            egui::RichText::new("Example recordings")
                                .strong()
                                .text_style(DesignTokens::heading_3())
                                .color(ui.tokens().text_strong),
                        ));
                    },
                );

                ui.add_space(TITLE_TO_GRID_VSPACE);

                egui::Grid::new("example_section_grid")
                    .spacing(grid_spacing)
                    .min_col_width(column_width)
                    .max_col_width(column_width)
                    .show(ui, |ui| {
                        re_tracing::profile_scope!("example_section_grid");

                        // Disable text selection so that hovering the example card only hovers the card
                        ui.style_mut().interaction.selectable_labels = false;

                        for row_of_examples in examples.chunks_mut(column_count) {
                            re_tracing::profile_scope!("row");
                            let description_height = row_of_examples
                                .iter()
                                .map(|example| {
                                    example.description_height(
                                        ui,
                                        column_width - 2.0 * DESCRIPTION_INNER_MARGIN.x,
                                    )
                                })
                                .fold(0.0, f32::max);

                            for example in row_of_examples {
                                re_tracing::profile_scope!("example");
                                let rect = egui::Rect::from_min_size(
                                    ui.cursor().min,
                                    egui::vec2(column_width, column_width / CARD_ASPECT_RATIO),
                                );
                                let response = ui.interact(
                                    rect,
                                    section_id.with(&example.desc.name),
                                    egui::Sense::click(),
                                );
                                ui.advance_cursor_after_rect(rect);

                                let mut card_ui = ui.new_child(
                                    egui::UiBuilder::new()
                                        .scope_id(
                                            section_id
                                                .with(("example_card_ui", &example.desc.name)),
                                        )
                                        .max_rect(rect)
                                        .layout(egui::Layout::top_down(egui::Align::Min)),
                                );
                                let source_clicked = self
                                    .entrance_animation
                                    .show(
                                        &mut card_ui,
                                        section_id.with(("example_card", &example.desc.name)),
                                        EXAMPLE_CARD_ANIMATION,
                                        |ui| {
                                            example.card_ui(
                                                ui,
                                                rect,
                                                description_height,
                                                response.hovered(),
                                            )
                                        },
                                    )
                                    .inner;

                                if response.clicked() && !source_clicked {
                                    // TODO(#5177): This workaround is needed to avoid the click to "leak"
                                    // through the UI, potentially causing some views (e.g. timeseries or time
                                    // panel to quit auto-zoom mode.
                                    ui.input_mut(|i| i.pointer = Default::default());

                                    ui.open_url(egui::output::OpenUrl {
                                        url: example.desc.rrd_url.clone(),
                                        new_tab: false,
                                    });
                                }
                            }

                            ui.end_row();
                        }
                    });
            });
        });
    }
}

impl ExampleDescLayout {
    fn image_uv(&self, display_rect: egui::Rect) -> egui::Rect {
        let image_width = self.desc.thumbnail.width as f32;
        let image_height = self.desc.thumbnail.height as f32;
        let display_aspect_ratio = display_rect.aspect_ratio();
        let image_aspect_ratio = image_width / image_height;
        if image_aspect_ratio > display_aspect_ratio {
            let a = (1.0 - display_aspect_ratio / image_aspect_ratio) / 2.0;
            egui::Rect::from_min_max(egui::Pos2::new(a, 0.0), egui::Pos2::new(1.0 - a, 1.0))
        } else {
            let a = (1.0 - image_aspect_ratio / display_aspect_ratio) / 2.0;
            egui::Rect::from_min_max(egui::Pos2::new(0.0, a), egui::Pos2::new(1.0, 1.0 - a))
        }
    }

    fn image(&self, uv: egui::Rect) -> egui::Image<'_> {
        egui::Image::new(&self.desc.thumbnail.url)
            .uv(uv)
            .corner_radius(CARD_RADIUS)
    }

    fn metadata_items(&self) -> Vec<MetadataItem<'_>> {
        let mut items = self
            .desc
            .tags
            .iter()
            .sorted()
            .map(|tag| MetadataItem::Tag(tag))
            .collect_vec();
        if let Some(source_url) = self.desc.source_url.as_deref() {
            items.push(MetadataItem::Source(source_url));
        }
        if let Some(Some(size)) = self.rrd_byte_size_promise.ready().copied() {
            items.push(MetadataItem::FileSize(re_format::format_bytes(size as f64)));
        }
        items
    }

    fn metadata_layout(&self, ui: &Ui, available_width: f32) -> MetadataLayout<'_> {
        let mut items = Vec::new();
        let mut cursor = egui::Vec2::ZERO;
        let mut row_height: f32 = 0.0;

        for item in self.metadata_items() {
            let size = item.size(ui, available_width);
            if 0.0 < cursor.x && available_width < cursor.x + size.x {
                cursor.x = 0.0;
                cursor.y += row_height + METADATA_ITEM_SPACING.y;
                row_height = 0.0;
            }

            let offset = cursor;
            cursor.x += size.x + METADATA_ITEM_SPACING.x;
            row_height = row_height.max(size.y);
            items.push(PositionedMetadataItem { item, offset, size });
        }

        MetadataLayout {
            items,
            height: cursor.y + row_height,
        }
    }

    fn description_height(&self, ui: &Ui, available_width: f32) -> f32 {
        let title_height = ui.text_style_height(&DesignTokens::heading_4());
        let metadata_height = self.metadata_layout(ui, available_width).height;
        (2.0 * DESCRIPTION_INNER_MARGIN.y
            + title_height
            + TITLE_TO_METADATA_VSPACE
            + metadata_height)
            .ceil()
    }

    fn metadata_ui(&self, ui: &mut Ui) -> bool {
        let available_width = ui.available_width();
        let layout = self.metadata_layout(ui, available_width);
        let origin = ui.cursor().min;
        let mut source_clicked = false;

        for (index, positioned) in layout.items.into_iter().enumerate() {
            let rect = egui::Rect::from_min_size(origin + positioned.offset, positioned.size);
            let id = ui.scope_id().with(("example_metadata", index));
            let mut item_ui = ui.new_child(
                egui::UiBuilder::new()
                    .scope_id(id.with("ui"))
                    .max_rect(rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Min)),
            );
            item_ui.set_clip_rect(ui.clip_rect().intersect(rect));

            let response = item_ui.add(positioned.item.atom_layout(ui, id));

            if let MetadataItem::Source(source_url) = positioned.item {
                let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
                if response.clicked() {
                    source_clicked = true;
                    ui.open_url(egui::output::OpenUrl {
                        url: source_url.to_owned(),
                        new_tab: true,
                    });
                }
            }
        }

        ui.advance_cursor_after_rect(egui::Rect::from_min_size(
            origin,
            egui::vec2(available_width, layout.height),
        ));
        source_clicked
    }

    fn card_ui(
        &self,
        ui: &mut Ui,
        card_rect: egui::Rect,
        description_height: f32,
        hovered: bool,
    ) -> bool {
        re_tracing::profile_function!();
        self.image(self.image_uv(card_rect)).paint_at(ui, card_rect);

        let description_rect = egui::Rect::from_min_max(
            egui::pos2(card_rect.left(), card_rect.bottom() - description_height),
            card_rect.right_bottom(),
        );
        let description_radius = egui::CornerRadius {
            nw: 0,
            ne: 0,
            sw: CARD_RADIUS,
            se: CARD_RADIUS,
        };
        ui.painter().rect_filled(
            description_rect,
            description_radius,
            ui.tokens().panel_bg_color.gamma_multiply(0.90),
        );

        if hovered {
            ui.painter().rect_filled(
                card_rect,
                CARD_RADIUS,
                ui.tokens().text_strong.gamma_multiply(0.06),
            );
        }
        ui.painter().rect_stroke(
            card_rect,
            CARD_RADIUS,
            ui.tokens().native_frame_stroke,
            egui::StrokeKind::Inside,
        );

        let content_rect = egui::Rect::from_min_max(
            description_rect.min + DESCRIPTION_INNER_MARGIN,
            description_rect.max - DESCRIPTION_INNER_MARGIN,
        );
        let mut main_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(content_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        main_ui.set_clip_rect(ui.clip_rect().intersect(content_rect));
        main_ui.set_width(content_rect.width());
        main_ui.spacing_mut().item_spacing.y = TITLE_TO_METADATA_VSPACE;
        main_ui.add(
            egui::Label::new(
                egui::RichText::new(&self.desc.title)
                    .strong()
                    .text_style(DesignTokens::heading_4())
                    .color(main_ui.tokens().text_strong),
            )
            .truncate(),
        );
        self.metadata_ui(&mut main_ui)
    }
}
