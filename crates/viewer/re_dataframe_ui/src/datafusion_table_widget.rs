use std::sync::Arc;

use arrow::datatypes::Field;
use datafusion::prelude::SessionContext;
use datafusion::sql::TableReference;
use egui::containers::menu::MenuConfig;
use egui::{Frame, Id, Margin, OpenUrl, RichText, TopBottomPanel, Ui, Widget as _};
use egui_table::{CellInfo, HeaderCellInfo};
use itertools::Itertools as _;
use re_format::{format_plural_s, format_uint};
use re_log::error;
use re_log_types::{EntryId, TimelineName, Timestamp};
use re_sorbet::{ColumnDescriptorRef, SorbetSchema};
use re_ui::egui_ext::response_ext::ResponseExt as _;
use re_ui::menu::menu_style;
use re_ui::{UiExt as _, icons};
use re_viewer_context::{
    AsyncRuntimeHandle, SystemCommand, SystemCommandSender as _, ViewerContext,
};

use crate::StreamingCacheTableProvider;
use crate::datafusion_adapter::{DataFusionAdapter, DataFusionQueryResult};
use crate::display_record_batch::DisplayColumn;
use crate::filters::{ColumnFilter, FilterState};
use crate::header_tooltip::column_header_tooltip_ui;
use crate::re_table::ReTable;
use crate::re_table_utils::{ColumnConfig, TableConfig};
use crate::table_blueprint::{
    ColumnBlueprint, EntryLinksSpec, SegmentLinksSpec, SortBy, SortDirection, TableBlueprint,
};
use crate::table_selection::TableSelectionState;
use crate::{DisplayRecordBatch, default_display_name_for_column};

struct Column<'a> {
    /// The ID of the column (based on it's corresponding [`re_sorbet::ColumnDescriptor`]).
    id: egui::Id,

    /// Reference to the descriptor of this column.
    desc: ColumnDescriptorRef<'a>,

    /// The blueprint of this column.
    blueprint: ColumnBlueprint,
}

impl Column<'_> {
    fn display_name(&self) -> String {
        self.blueprint
            .display_name
            .clone()
            .unwrap_or_else(|| default_display_name_for_column(&self.desc))
    }
}

/// Keep track of a [`re_sorbet::SorbetBatch`]'s columns, along with their order and their blueprint.
struct Columns<'a> {
    columns: Vec<Column<'a>>,
}

impl<'a> Columns<'a> {
    fn from(sorbet_schema: &'a SorbetSchema, column_blueprint_fn: &ColumnBlueprintFn<'_>) -> Self {
        let columns = sorbet_schema
            .columns
            .iter()
            .map(|desc| {
                let id = egui::Id::new(desc);
                let desc = desc.into();
                let blueprint = column_blueprint_fn(&desc);

                Column {
                    id,
                    desc,
                    blueprint,
                }
            })
            .collect();

        Self { columns }
    }
}

impl Columns<'_> {
    fn iter(&self) -> impl Iterator<Item = &Column<'_>> + use<'_> {
        self.columns.iter()
    }
}

/// In which state the table currently is?
///
/// This is primarily useful for testing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableStatus {
    /// The table is loading its content for the first time and has no cached content. A spinner
    /// is displayed.
    InitialLoading,

    /// The table is fully loaded and no update is in progress.
    Loaded,

    /// The table is currently updating its content and a spinner is displayed. The previously loaded
    /// content is displayed in the meantime.
    Updating,

    /// An error occurred while loading the table. It is displayed in the UI with no additional
    /// content.
    Error(String),
}

type ColumnBlueprintFn<'a> = Box<dyn Fn(&ColumnDescriptorRef<'_>) -> ColumnBlueprint + 'a>;

pub struct DataFusionTableWidget<'a> {
    session_ctx: Arc<SessionContext>,
    table_ref: TableReference,

    /// If provided, add a title UI on top of the table.
    //TODO(ab): for now, this is the only way to have the column visibility/order menu
    title: Option<String>,

    /// If provided, this will add a "copy URL" button next to the title (which must be provided).
    url: Option<String>,

    /// User-provided closure to provide column blueprint.
    column_blueprint_fn: ColumnBlueprintFn<'a>,

    /// The blueprint used the first time the table is queried.
    initial_blueprint: TableBlueprint,
}

impl<'a> DataFusionTableWidget<'a> {
    /// Clears all caches related to this session context and table reference.
    pub fn refresh(
        runtime: &AsyncRuntimeHandle,
        egui_ctx: &egui::Context,
        session_ctx: Arc<SessionContext>,
        table_ref: impl Into<TableReference>,
    ) {
        let table_ref = table_ref.into();
        let id = id_from_session_context_and_table(&session_ctx, &table_ref);

        // Clear UI state
        DataFusionAdapter::clear_state(egui_ctx, id);

        // Clear the underlying StreamingCacheTableProvider cache if present
        runtime.spawn_future(async move {
            if let Ok(provider) = session_ctx.table_provider(table_ref).await {
                if let Some(cache_provider) = provider
                    .as_any()
                    .downcast_ref::<StreamingCacheTableProvider>()
                {
                    cache_provider.refresh();
                }
            }
        });
    }

    pub fn new(session_ctx: Arc<SessionContext>, table_ref: impl Into<TableReference>) -> Self {
        Self {
            session_ctx,
            table_ref: table_ref.into(),

            title: None,
            url: None,
            column_blueprint_fn: Box::new(|_| ColumnBlueprint::default()),
            initial_blueprint: Default::default(),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());

        self
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());

        self
    }

    pub fn column_blueprint(
        mut self,
        column_blueprint_fn: impl Fn(&ColumnDescriptorRef<'_>) -> ColumnBlueprint + 'a,
    ) -> Self {
        self.column_blueprint_fn = Box::new(column_blueprint_fn);

        self
    }

    pub fn initial_blueprint(mut self, initial_blueprint: TableBlueprint) -> Self {
        self.initial_blueprint = initial_blueprint;
        self
    }

    pub fn generate_segment_links(
        mut self,
        column_name: impl Into<String>,
        segment_id_column_name: impl Into<String>,
        origin: re_uri::Origin,
        dataset_id: EntryId,
    ) -> Self {
        self.initial_blueprint.segment_links = Some(SegmentLinksSpec {
            column_name: column_name.into(),
            segment_id_column_name: segment_id_column_name.into(),
            origin,
            dataset_id,
        });

        self
    }

    pub fn generate_entry_links(
        mut self,
        column_name: impl Into<String>,
        entry_id_column_name: impl Into<String>,
        origin: re_uri::Origin,
    ) -> Self {
        self.initial_blueprint.entry_links = Some(EntryLinksSpec {
            column_name: column_name.into(),
            entry_id_column_name: entry_id_column_name.into(),
            origin,
        });

        self
    }

    pub fn prefilter(mut self, expression: datafusion::prelude::Expr) -> Self {
        self.initial_blueprint.prefilter = Some(expression);
        self
    }

    /// Display the table.
    pub fn show(
        self,
        viewer_ctx: &ViewerContext<'_>,
        runtime: &AsyncRuntimeHandle,
        ui: &mut egui::Ui,
    ) -> TableStatus {
        let Self {
            session_ctx,
            table_ref,
            title,
            url,
            column_blueprint_fn,
            initial_blueprint,
        } = self;

        match session_ctx.table_exist(table_ref.clone()) {
            Ok(true) => {}
            Ok(false) => {
                ui.loading_screen(
                    "Loading table:",
                    url.as_deref().or(title.as_deref()).unwrap_or(""),
                );
                return TableStatus::InitialLoading;
            }
            Err(err) => {
                ui.loading_screen(
                    "Error while loading table:",
                    RichText::from(err.to_string()).color(ui.style().visuals.error_fg_color),
                );
                return TableStatus::Error(err.to_string());
            }
        }

        // The TableConfig should be persisted across sessions, so we also need a static id.
        let session_id = id_from_session_context_and_table(&session_ctx, &table_ref);
        let table_state = DataFusionAdapter::get(
            runtime,
            ui,
            &session_ctx,
            table_ref.clone(),
            session_id,
            initial_blueprint,
        );

        let requested_query_result = table_state.results.as_ref();

        let is_table_update_in_progress;
        let query_result = match (requested_query_result, &table_state.last_query_results) {
            (Some(Ok(query_result)), _) => {
                is_table_update_in_progress = !query_result.finished;
                query_result
            }

            (Some(Err(err)), _) => {
                let error = format!("Could not load table: {err}");

                ui.horizontal(|ui| {
                    ui.error_label(&error);

                    if ui
                        .small_icon_button(&re_ui::icons::RESET, "Refresh")
                        .clicked()
                    {
                        // This will trigger a fresh query on the next frame.
                        Self::refresh(runtime, ui.ctx(), Arc::clone(&session_ctx), table_ref);
                    }
                });
                return TableStatus::Error(error);
            }

            (None, Some(Ok(last_query_result))) => {
                // The new dataframe is still processing, but we have the previous one to display for now.
                is_table_update_in_progress = true;
                last_query_result
            }

            (None, None | Some(Err(_))) => {
                // still processing, nothing yet to show
                //TODO(ab): it can happen that we're stuck in the state. We should detect it and
                //produce an error
                ui.loading_screen(
                    "Loading table:",
                    url.as_deref().or(title.as_deref()).unwrap_or(""),
                );
                return TableStatus::InitialLoading;
            }
        };

        let new_blueprint = Self::table_ui(
            viewer_ctx,
            runtime,
            ui,
            Arc::clone(&session_ctx),
            table_ref,
            table_state.blueprint(),
            session_id,
            title.as_deref(),
            url.as_deref(),
            table_state.queried_at,
            is_table_update_in_progress,
            query_result,
            &column_blueprint_fn,
        );

        if table_state.blueprint() != &new_blueprint {
            table_state.update_query(runtime, ui, new_blueprint);
        }

        if is_table_update_in_progress {
            TableStatus::Updating
        } else {
            TableStatus::Loaded
        }
    }

    /// Actual UI code to render a table.
    //TODO(ab): make the argument list less crazy
    #[expect(clippy::too_many_arguments)]
    fn table_ui(
        viewer_ctx: &ViewerContext<'_>,
        runtime: &AsyncRuntimeHandle,
        ui: &mut egui::Ui,
        session_ctx: Arc<SessionContext>,
        table_ref: TableReference,
        table_blueprint: &TableBlueprint,
        session_id: egui::Id,
        title: Option<&str>,
        url: Option<&str>,
        queried_at: Timestamp,
        should_show_spinner: bool,
        query_result: &DataFusionQueryResult,
        column_blueprint_fn: &ColumnBlueprintFn<'_>,
    ) -> TableBlueprint {
        let static_id = Id::new(&table_ref);

        let mut new_blueprint = table_blueprint.clone();

        let mut filter_state =
            FilterState::load_or_init_from_blueprint(ui.ctx(), session_id, table_blueprint);

        let num_rows = query_result
            .sorbet_batches
            .iter()
            .map(|record_batch| record_batch.num_rows() as u64)
            .sum();

        let columns = Columns::from(&query_result.sorbet_schema, column_blueprint_fn);

        let display_record_batches = query_result
            .sorbet_batches
            .iter()
            .map(|record_batch| {
                DisplayRecordBatch::try_new(itertools::izip!(
                    query_result.sorbet_schema.columns.iter().map(|x| x.into()),
                    columns.iter().map(|column| &column.blueprint),
                    record_batch.columns().iter().map(Arc::clone)
                ))
            })
            .collect::<Result<Vec<_>, _>>();

        let display_record_batches = match display_record_batches {
            Ok(display_record_batches) => display_record_batches,
            Err(err) => {
                //TODO(ab): better error handling?
                ui.error_label(err.to_string());
                return new_blueprint;
            }
        };

        let mut table_config = TableConfig::get_with_columns(
            ui.ctx(),
            static_id,
            columns.iter().map(|column| {
                ColumnConfig::new_with_visible(
                    column.id,
                    column.display_name(),
                    column.blueprint.default_visibility,
                )
                .with_sort_key(column.blueprint.sort_key)
            }),
        );

        if let Some(title) = title {
            title_ui(
                ui,
                viewer_ctx,
                Some(&mut table_config),
                title,
                url,
                should_show_spinner,
            );
        }

        filter_state.filter_bar_ui(
            ui,
            viewer_ctx.app_options().timestamp_format,
            &mut new_blueprint,
        );

        let table_style = re_ui::TableStyle::Spacious;

        let mut row_height = viewer_ctx.tokens().table_row_height(table_style);

        // If the first column is a blob, we treat it as a thumbnail and increase the row height.
        // TODO(lucas): This is a band-aid fix and should be replaced with proper table blueprint
        let first_column = table_config
            .visible_column_indexes()
            .next()
            .and_then(|index| display_record_batches.first()?.columns().get(index));
        if let Some(DisplayColumn::Component(component)) = first_column
            && component.is_image()
        {
            row_height *= 3.0;
        }

        let migrated_fields = query_result
            .sorbet_schema
            .columns
            .arrow_fields(re_sorbet::BatchType::Dataframe);

        let mut table_delegate = DataFusionTableDelegate {
            session_id,
            ctx: viewer_ctx,
            table_style,
            query_result,
            migrated_fields: &migrated_fields,
            display_record_batches: &display_record_batches,
            columns: &columns,
            blueprint: table_blueprint,
            new_blueprint: &mut new_blueprint,
            filter_state: &mut filter_state,
            row_height,
        };

        let visible_columns = table_config.visible_columns().count();
        let total_columns = columns.columns.len();

        let action = Self::bottom_bar_ui(
            ui,
            viewer_ctx,
            session_id,
            num_rows,
            visible_columns,
            total_columns,
            queried_at,
        );

        match action {
            Some(BottomBarAction::Refresh) => {
                Self::refresh(runtime, ui.ctx(), session_ctx, table_ref);
            }
            None => {}
        }
        ReTable::new(
            ui.ctx(),
            session_id,
            &mut table_delegate,
            &table_config,
            num_rows,
        )
        .show(ui);

        table_config.store(ui.ctx());
        filter_state.store(ui.ctx(), session_id);

        new_blueprint
    }

    fn bottom_bar_ui(
        ui: &mut Ui,
        ctx: &ViewerContext<'_>,
        session_id: Id,
        total_rows: u64,
        visible_columns: usize,
        total_columns: usize,
        queried_at: Timestamp,
    ) -> Option<BottomBarAction> {
        let mut action = None;

        let frame = Frame::new()
            .fill(ui.tokens().table_header_bg_fill)
            .inner_margin(Margin::symmetric(12, 0));
        TopBottomPanel::bottom(session_id.with("bottom_bar"))
            .frame(frame)
            .show_separator_line(false)
            .show_inside(ui, |ui| {
                let height = 24.0;
                ui.set_height(height);
                ui.horizontal_centered(|ui| {
                    ui.visuals_mut().widgets.noninteractive.fg_stroke.color =
                        ui.tokens().text_subdued;
                    ui.visuals_mut().widgets.active.fg_stroke.color = ui.tokens().text_default;

                    egui::Sides::new().show(
                        ui,
                        |ui| {
                            ui.set_height(height);

                            ui.label("rows:");
                            ui.strong(format_uint(total_rows));

                            ui.add_space(16.0);

                            ui.label("columns:");
                            ui.strong(format!(
                                "{} out of {}",
                                format_uint(visible_columns),
                                format_uint(total_columns),
                            ));
                        },
                        |ui| {
                            ui.set_height(height);
                            if icons::RESET.as_button().ui(ui).clicked() {
                                action = Some(BottomBarAction::Refresh);
                            }

                            re_ui::time::short_duration_ui(
                                ui,
                                queried_at,
                                ctx.app_options().timestamp_format,
                                Ui::strong,
                            );
                            ui.label("Last updated:");
                        },
                    );
                });
            });

        action
    }
}

fn id_from_session_context_and_table(
    session_ctx: &SessionContext,
    table_ref: &TableReference,
) -> Id {
    egui::Id::new((session_ctx.session_id(), table_ref))
}

fn title_ui(
    ui: &mut egui::Ui,
    ctx: &ViewerContext<'_>,
    table_config: Option<&mut TableConfig>,
    title: &str,
    url: Option<&str>,
    should_show_spinner: bool,
) {
    Frame::new()
        .inner_margin(Margin {
            top: 16,
            bottom: 12,
            left: 16,
            right: 16,
        })
        .show(ui, |ui| {
            egui::Sides::new().show(
                ui,
                |ui| {
                    ui.heading(RichText::new(title).strong());
                    if let Some(url) = url
                        && ui
                            .small_icon_button(&re_ui::icons::COPY, "Copy URL")
                            .on_hover_text(url)
                            .clicked()
                    {
                        ctx.command_sender()
                            .send_system(SystemCommand::CopyViewerUrl(url.to_owned()));
                    }

                    if should_show_spinner {
                        ui.spinner();
                    }
                },
                |ui| {
                    if let Some(table_config) = table_config {
                        table_config.button_ui(ui);
                    }
                },
            );
        });
}

enum BottomBarAction {
    Refresh,
}

struct DataFusionTableDelegate<'a> {
    session_id: Id,
    ctx: &'a ViewerContext<'a>,
    table_style: re_ui::TableStyle,
    query_result: &'a DataFusionQueryResult,
    migrated_fields: &'a Vec<Field>,
    display_record_batches: &'a Vec<DisplayRecordBatch>,
    columns: &'a Columns<'a>,
    blueprint: &'a TableBlueprint,
    new_blueprint: &'a mut TableBlueprint,
    filter_state: &'a mut FilterState,
    row_height: f32,
}

impl DataFusionTableDelegate<'_> {
    /// Find the record batch and local row index for a global row index.
    pub fn with_row_batch(
        batches: &[DisplayRecordBatch],
        mut row_index: usize,
    ) -> Option<(&DisplayRecordBatch, usize)> {
        for batch in batches {
            let row_count = batch.num_rows();
            if row_index < row_count {
                return Some((batch, row_index));
            } else {
                row_index -= row_count;
            }
        }
        None
    }

    fn segment_link_for_row(&self, row: u64, spec: &SegmentLinksSpec) -> Option<String> {
        let (display_record_batch, batch_index) =
            Self::with_row_batch(self.display_record_batches, row as usize)?;
        let column_index = self
            .columns
            .iter()
            .position(|col| col.blueprint.display_name.as_ref() == Some(&spec.column_name))?;
        let column = display_record_batch.columns().get(column_index)?;

        match column {
            DisplayColumn::RowId { .. } | DisplayColumn::Timeline { .. } => None,
            DisplayColumn::Component(col) => col.string_value_at(batch_index),
        }
    }

    pub fn row_context_menu(&self, ui: &Ui, _row_number: u64) {
        let has_context_menu = self.blueprint.segment_links.is_some();
        if !has_context_menu {
            return;
        }

        ui.response().container_context_menu(|ui| {
            let selection = TableSelectionState::load(ui.ctx(), self.session_id);

            // re_table will ensure that the right-clicked row is always selected.
            let selected_rows = selection.selected_rows;

            if let Some(segment_links_spec) = &self.blueprint.segment_links {
                let label = format!(
                    "Open {} segment{}",
                    selected_rows.len(),
                    format_plural_s(selected_rows.len())
                );
                let response =
                    ui.add(icons::OPEN_RECORDING.as_button_with_label(ui.tokens(), label));

                let open = |new_tab| {
                    // Let's open the recordings in order
                    for row in selected_rows.iter().copied().sorted() {
                        if let Some(segment_link) =
                            self.segment_link_for_row(row, segment_links_spec)
                        {
                            ui.ctx().open_url(OpenUrl {
                                url: segment_link,
                                new_tab,
                            });
                        } else {
                            error!("Could not get segment link for row {}", row);
                        }
                    }
                };

                if response.clicked_with_open_in_background() {
                    open(true);
                } else if response.clicked() {
                    open(false);
                }
            }
        });
    }
}

impl egui_table::TableDelegate for DataFusionTableDelegate<'_> {
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &HeaderCellInfo) {
        let tokens = ui.tokens();
        let table_style = self.table_style;
        let col_index = cell.group_index;
        if let Some(column) = self.columns.columns.get(col_index) {
            let column_field = &self.query_result.original_schema.fields[col_index];
            let column_physical_name = column_field.name();
            let column_display_name = column.display_name();

            let current_sort_direction = self.blueprint.sort_by.as_ref().and_then(|sort_by| {
                (sort_by.column_physical_name.as_str() == column_physical_name)
                    .then_some(&sort_by.direction)
            });

            egui::Sides::new()
                .shrink_left()
                .show(
                    ui,
                    |ui| {
                        ui.set_height(ui.tokens().table_content_height(table_style));
                        let response = ui.label(
                            egui::RichText::new(column_display_name)
                                .strong()
                                .monospace(),
                        );

                        if let Some(dir_icon) = current_sort_direction.map(SortDirection::icon) {
                            ui.add_space(-5.0);
                            ui.small_icon(dir_icon, Some(tokens.table_sort_icon_color));
                        }

                        response
                    },
                    |ui| {
                        ui.set_height(ui.tokens().table_content_height(table_style));
                        egui::containers::menu::MenuButton::from_button(
                            ui.small_icon_button_widget(&re_ui::icons::MORE, "More options"),
                        )
                        .config(MenuConfig::new().style(menu_style()))
                        .ui(ui, |ui| {
                            for sort_direction in SortDirection::iter() {
                                let already_sorted =
                                    Some(&sort_direction) == current_sort_direction;

                                if ui
                                    .add_enabled_ui(!already_sorted, |ui| {
                                        sort_direction.menu_item_ui(ui)
                                    })
                                    .inner
                                    .clicked()
                                {
                                    self.new_blueprint.sort_by = Some(SortBy {
                                        column_physical_name: column_physical_name.to_owned(),
                                        direction: sort_direction,
                                    });
                                    ui.close();
                                }
                            }

                            // TODO(ab): for now, we disable filtering on any column with a
                            // variant UI, because chances are the filter will not be
                            // relevant to what's displayed (e.g. recording link column).
                            // In the future, we'll probably need to be more fine-grained.
                            #[expect(clippy::collapsible_if)]
                            if column.blueprint.variant_ui.is_none()
                                && let Some(column_filter) =
                                    ColumnFilter::default_for_column(Arc::clone(column_field))
                            {
                                if ui
                                    .icon_and_text_menu_item(&re_ui::icons::FILTER, "Filter")
                                    .clicked()
                                {
                                    self.filter_state.push_new_filter(column_filter);
                                }
                            }
                        });
                    },
                )
                .0
                .on_hover_ui(|ui| {
                    ui.with_optional_extras(|ui, show_extras| {
                        column_header_tooltip_ui(
                            ui,
                            &column.desc,
                            column_field,
                            &self.migrated_fields[col_index],
                            show_extras,
                        );
                    });
                });
        }
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &CellInfo) {
        let col_index = cell.col_nr;

        if let Some((display_record_batch, batch_index)) =
            Self::with_row_batch(self.display_record_batches, cell.row_nr as usize)
        {
            let column = &display_record_batch.columns()[col_index];

            // TODO(#9029): it is _very_ unfortunate that we must provide a fake timeline, but
            // avoiding doing so needs significant refactoring work.
            column.data_ui(
                self.ctx,
                ui,
                &re_viewer_context::external::re_chunk_store::LatestAtQuery::latest(
                    TimelineName::new("unknown"),
                ),
                batch_index,
                None,
            );
        }
    }

    fn row_ui(&mut self, ui: &mut Ui, row_nr: u64) {
        self.row_context_menu(ui, row_nr);
    }

    fn default_row_height(&self) -> f32 {
        self.row_height
    }
}
