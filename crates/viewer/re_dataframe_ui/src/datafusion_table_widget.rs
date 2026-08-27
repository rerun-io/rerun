use std::borrow::Cow;
use std::sync::Arc;

use arrow::array::{Array as _, BooleanArray};
use arrow::datatypes::Field;
use datafusion::prelude::SessionContext;
use datafusion::sql::TableReference as DataFusionTableReference;
use egui::containers::menu::MenuConfig;
use egui::{Frame, Id, Margin, OpenUrl, Panel, RichText, Ui};
use egui_table::{CellInfo, HeaderCellInfo};
use itertools::Itertools as _;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_async::AsyncRuntimeHandle;
use re_format::{format_plural_s, format_uint};
use re_log::error;
use re_log_types::{EntryId, Timestamp};
use re_protos::cloud::v1alpha1::ext;
use re_sdk_types::blueprint::components::ColumnName;
use re_sorbet::{ColumnDescriptorRef, SorbetSchema};
use re_ui::egui_ext::response_ext::ResponseExt as _;
use re_ui::menu::menu_style;
use re_ui::{UiExt as _, UiLayout, icons};
use re_viewer_context::{AppContext, SystemCommand, SystemCommandSender as _, TableReference};

use crate::cards_view::FlagChangeEvent;
use crate::column_sorting::{SortBy, SortDirection};
use crate::datafusion_adapter::{DataFusionAdapter, DataFusionQueryData, DataFusionQueryResult};
use crate::display_record_batch::DisplayColumn;
use crate::filters::{ColumnFilter, FilterState};
use crate::header_tooltip::column_header_tooltip_ui;
use crate::preview_renderer::PreviewRecording;
use crate::re_table::ReTable;
use crate::re_table_utils::UiTableConfig;
use crate::table_blueprint::{EntryLinksSpec, SegmentLinksSpec, TableBlueprint};
use crate::table_selection::TableSelectionState;
use crate::{ColumnBlueprint, DisplayRecordBatch, default_display_name_for_column};
use crate::{StreamingCacheTableProvider, TableBlueprints};

/// Minimum row height (in points) when segment preview views are shown.
const SEGMENT_PREVIEW_SIZE: f32 = 200.0;

/// Whether the table is shown as a traditional table or as cards.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum TableViewMode {
    #[default]
    Table,
    Cards,
}

/// Output produced by [`DataFusionTableWidget::table_ui`].
struct TableUiOutput {
    /// The (potentially modified) query state.
    query_data: DataFusionQueryData,

    /// Resolved source information for flag mutation and write-back.
    flag_column: Option<ResolvedFlagColumn>,

    /// Flag toggle changes from the card layout.
    flag_changes: Vec<FlagChangeEvent>,
}

struct ResolvedFlagColumn {
    display_index: usize,
    flag_field: Arc<Field>,
    index_field: Arc<Field>,
}

pub struct Column<'a> {
    /// The original Arrow field name used by DataFusion to identify the column.
    ///
    /// [`ColumnName`] always refers to this original name, never a name produced by Sorbet
    /// migration.
    physical_name: ColumnName,

    /// Reference to the descriptor of this column.
    pub desc: ColumnDescriptorRef<'a>,

    /// The blueprint of this column.
    pub blueprint: ColumnBlueprint,
}

impl Column<'_> {
    /// Returns the original Arrow field name used by DataFusion.
    pub fn physical_name(&self) -> &ColumnName {
        &self.physical_name
    }

    /// Returns the display name of the column.
    ///
    /// Do not use it to identify the column; use [`Self::physical_name`] instead.
    pub fn display_name(&self) -> String {
        self.blueprint
            .display_name
            .clone()
            .unwrap_or_else(|| default_display_name_for_column(&self.desc))
    }

    fn sort_by(&self, direction: SortDirection) -> SortBy {
        SortBy {
            column_name: self.physical_name.clone(),
            direction,
        }
    }
}

/// Keep track of a [`re_sorbet::SorbetBatch`]'s columns, along with their order and their blueprint.
pub struct Columns<'a> {
    pub columns: Vec<Column<'a>>,
}

impl<'a> Columns<'a> {
    fn from(
        sorbet_schema: &'a SorbetSchema,
        original_schema: &arrow::datatypes::Schema,
        column_blueprint_fn: &ColumnBlueprintFn<'_>,
    ) -> Self {
        re_log::debug_assert_eq!(original_schema.fields().len(), sorbet_schema.columns.len());

        // TODO(andreas): Preserve the DataFusion field name in the Sorbet schema so this mapping
        // does not depend on migration preserving column order.
        let columns = sorbet_schema
            .columns
            .iter()
            .enumerate()
            .map(|(index, desc)| {
                let physical_name = original_schema.fields()[index].name().clone().into();
                let desc = desc.into();
                let blueprint = column_blueprint_fn(&desc);

                Column {
                    physical_name,
                    desc,
                    blueprint,
                }
            })
            .collect();

        Self { columns }
    }
}

impl Columns<'_> {
    pub fn iter(&self) -> impl Iterator<Item = &Column<'_>> + use<'_> {
        self.columns.iter()
    }

    /// Find a column by its physical name.
    pub fn find_by_physical_name(&self, name: &ColumnName) -> Option<(usize, &Column<'_>)> {
        self.columns
            .iter()
            .enumerate()
            .find(|(_, c)| c.physical_name() == name)
    }

    /// Find a column index by its physical name.
    pub fn index_by_physical_name(&self, name: &ColumnName) -> Option<usize> {
        self.find_by_physical_name(name).map(|(idx, _)| idx)
    }
}

/// In which state the table currently is?
///
/// This is primarily useful for testing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableStatus {
    /// The table is loading its content for the first time and has no cached content.
    /// A loading indicator is displayed.
    InitialLoading,

    /// The table is fully loaded and no update is in progress.
    Loaded,

    /// The table is currently updating its content and a loading indicator is displayed. The previously loaded
    /// content is displayed in the meantime.
    Updating,

    /// An error occurred while loading the table. It is displayed in the UI with no additional
    /// content.
    Error(String),
}

type ColumnBlueprintFn<'a> = Box<dyn Fn(&ColumnDescriptorRef<'_>) -> ColumnBlueprint + 'a>;

pub struct DataFusionTableWidget<'a> {
    session_ctx: Arc<SessionContext>,

    /// Stable identity used by table-scoped actions.
    table_ref: TableReference,

    datafusion_table_ref: DataFusionTableReference,

    /// If provided, the toolbar on top of the table shows this as its title.
    title: Option<String>,

    /// User-provided closure to provide column blueprint.
    column_blueprint_fn: ColumnBlueprintFn<'a>,

    /// Query state used only when creating the egui-owned adapter for the first time.
    initial_query_data: DataFusionQueryData,
}

impl<'a> DataFusionTableWidget<'a> {
    /// Clears all caches related to this session context and table reference.
    pub fn refresh(
        runtime: &AsyncRuntimeHandle,
        egui_ctx: egui::Context,
        session_ctx: Arc<SessionContext>,
        table_ref: impl Into<DataFusionTableReference>,
    ) {
        let table_ref = table_ref.into();

        // Unfortunately, getting a TableProvider is async, so we need to spawn here:
        runtime.spawn_future(async move {
            Self::invalidate_streaming_cache(&session_ctx, &table_ref).await;

            let id = id_from_session_context_and_table(&session_ctx, &table_ref);
            DataFusionAdapter::clear_state(&egui_ctx, id);
        });
    }

    /// Invalidate the streaming cache so the next query re-fetches from the server.
    ///
    /// Unlike [`Self::refresh`], this does NOT clear the adapter state, so the current
    /// query results remain visible until a new query completes.
    async fn invalidate_streaming_cache(
        session_ctx: &SessionContext,
        table_ref: &DataFusionTableReference,
    ) {
        if let Ok(provider) = session_ctx.table_provider(table_ref.clone()).await
            && let Some(cache_provider) = provider.downcast_ref::<StreamingCacheTableProvider>()
        {
            cache_provider.refresh();
        }
    }

    pub fn new(
        session_ctx: Arc<SessionContext>,
        datafusion_table_ref: impl Into<DataFusionTableReference>,
        table_ref: TableReference,
    ) -> Self {
        Self {
            session_ctx,
            table_ref,
            datafusion_table_ref: datafusion_table_ref.into(),

            title: None,
            column_blueprint_fn: Box::new(|_| ColumnBlueprint::default()),
            initial_query_data: Default::default(),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());

        self
    }

    pub fn column_blueprint(
        mut self,
        column_blueprint_fn: impl Fn(&ColumnDescriptorRef<'_>) -> ColumnBlueprint + 'a,
    ) -> Self {
        self.column_blueprint_fn = Box::new(column_blueprint_fn);

        self
    }

    /// Set the initial sort used when no egui-owned adapter state exists yet.
    pub fn sort_by(mut self, sort_by: SortBy) -> Self {
        self.initial_query_data.sort_by = Some(sort_by);
        self
    }

    pub fn generate_segment_links(
        mut self,
        column_name: ColumnName,
        segment_id_column_name: ColumnName,
        origin: re_uri::Origin,
        dataset_id: EntryId,
    ) -> Self {
        self.initial_query_data.segment_links = Some(SegmentLinksSpec {
            column_name,
            segment_id_column_name,
            origin,
            dataset_id,
        });

        self
    }

    pub fn generate_entry_links(
        mut self,
        column_name: ColumnName,
        entry_id_column_name: ColumnName,
        origin: re_uri::Origin,
    ) -> Self {
        self.initial_query_data.entry_links = Some(EntryLinksSpec {
            column_name,
            entry_id_column_name,
            origin,
        });

        self
    }

    pub fn prefilter(mut self, expression: datafusion::prelude::Expr) -> Self {
        self.initial_query_data.prefilter = Some(expression);
        self
    }

    fn display_name(&self) -> Cow<'_, str> {
        self.title
            .as_deref()
            .map(Cow::Borrowed)
            .or_else(|| self.table_ref.url().map(|url| Cow::Owned(url.to_string())))
            .unwrap_or_default()
    }

    /// Resolve the original Arrow field for the configured flag column if flagging is available.
    ///
    /// Requires all of:
    /// - `flag_column` set in the blueprint
    /// - The named column exists in the table as a boolean
    /// - Remote tables have a table-index column for write-back upserts
    /// - The token for the remote table (if any) has write permission
    fn resolve_flag_column(
        blueprint: &TableBlueprint,
        columns: &Columns<'_>,
        original_schema: &arrow::datatypes::Schema,
        remote_table: Option<&re_uri::EntryUri>,
        connection_registry: &re_redap_client::ConnectionRegistryHandle,
    ) -> Option<ResolvedFlagColumn> {
        let Some(flag_column_name) = &blueprint.flag_column else {
            return None;
        };

        let Some(remote_table) = remote_table else {
            // Local tables don't support flagging for now.
            return None;
        };

        let Some((display_index, column)) = columns.find_by_physical_name(flag_column_name) else {
            re_log::warn_once!("Flag column {flag_column_name:?} does not exist in the table");
            return None;
        };

        if !matches!(&column.desc, re_sorbet::ColumnDescriptorRef::Component(component)
                    if component.store_datatype == arrow::datatypes::DataType::Boolean)
        {
            re_log::warn_once!(
                "Flag column {flag_column_name:?} is not a boolean column or does not exist in the table"
            );
            return None;
        }

        let Some(flag_field) = original_schema
            .fields()
            .iter()
            .find(|field| field.name() == flag_column_name.as_str())
        else {
            re_log::warn_once!(
                "Flag column {flag_column_name:?} is missing from the original schema"
            );
            return None;
        };

        let Some((_, index_field)) = table_index_column(original_schema) else {
            re_log::warn_once!(
                "Flagging is disabled because remote table has no rerun:is_table_index column for upserts"
            );
            return None;
        };

        // Check write permission on the token for the remote table's origin.
        // `None` means unknown (e.g. stored credentials or no auth) — allow flagging.
        let has_write = connection_registry
            .credentials(&remote_table.origin)
            .and_then(|creds| creds.has_write_permission())
            .unwrap_or(true);

        if !has_write {
            return None;
        }

        Some(ResolvedFlagColumn {
            display_index,
            flag_field: Arc::clone(flag_field),
            index_field: Arc::clone(index_field),
        })
    }

    /// Displays the table.
    pub fn show(
        self,
        app_ctx: &AppContext<'_>,
        runtime: &AsyncRuntimeHandle,
        ui: &mut egui::Ui,
        table_blueprints: &TableBlueprints,
        view_states: &mut re_viewer_context::ViewStates,
    ) -> TableStatus {
        match self
            .session_ctx
            .table_exist(self.datafusion_table_ref.clone())
        {
            Ok(true) => {}
            Ok(false) => {
                ui.loading_screen("Loading table:", self.display_name().as_ref());
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
        let session_id =
            id_from_session_context_and_table(&self.session_ctx, &self.datafusion_table_ref);
        let mut table_state = DataFusionAdapter::get(
            runtime,
            ui,
            &self.session_ctx,
            self.datafusion_table_ref.clone(),
            session_id,
            self.initial_query_data.clone(),
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
                        Self::refresh(
                            runtime,
                            ui.ctx().clone(),
                            Arc::clone(&self.session_ctx),
                            self.datafusion_table_ref.clone(),
                        );
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
                ui.loading_screen("Loading table:", self.display_name().as_ref());
                return TableStatus::InitialLoading;
            }
        };

        let output = self.table_ui(
            app_ctx,
            runtime,
            ui,
            table_state.query_data(),
            session_id,
            table_state.queried_at,
            is_table_update_in_progress,
            query_result,
            table_blueprints,
            view_states,
        );

        // Flag changes are only produced when flagging_enabled was true in table_ui,
        // which already validated: flag_column is Some and column exists as boolean.
        if !output.flag_changes.is_empty()
            && let Some(flag_col) = &output.flag_column
        {
            table_state.apply_flag_changes(ui, flag_col.display_index, &output.flag_changes);

            if let Some(re_uri::RedapUri::Entry(entry_uri)) = &self.table_ref.url()
                && let Some(Ok(results)) = &table_state.results
            {
                upsert_flag_changes(
                    app_ctx,
                    runtime,
                    entry_uri,
                    results,
                    flag_col,
                    &output.flag_changes,
                );
            }

            // Invalidate the streaming cache so the next re-query (e.g. filter/sort change)
            // fetches fresh data from the server (which now has the upserted flags).
            let session_ctx = Arc::clone(&self.session_ctx);
            let table_ref = self.datafusion_table_ref.clone();
            runtime.spawn_future(async move {
                Self::invalidate_streaming_cache(&session_ctx, &table_ref).await;
            });
        }

        if table_state.query_data() != &output.query_data {
            table_state.update_query(runtime, ui, output.query_data);
        }

        if is_table_update_in_progress {
            TableStatus::Updating
        } else {
            TableStatus::Loaded
        }
    }

    /// Actual UI code to render a table.
    fn table_ui(
        &self,
        ctx: &AppContext<'_>,
        runtime: &AsyncRuntimeHandle,
        ui: &mut egui::Ui,
        query_data: &DataFusionQueryData,
        session_id: egui::Id,
        queried_at: Timestamp,
        should_show_loading_indicator: bool,
        query_result: &DataFusionQueryResult,
        table_blueprints: &TableBlueprints,
        view_states: &mut re_viewer_context::ViewStates,
    ) -> TableUiOutput {
        let static_id = Id::new(&self.datafusion_table_ref);

        let mut query_data = query_data.clone();

        let mut filter_state = FilterState::load_or_init_from_filters(
            ui.ctx(),
            session_id,
            &query_data.column_filters,
        );

        let num_rows = query_result
            .sorbet_batches
            .iter()
            .map(|record_batch| record_batch.num_rows() as u64)
            .sum();

        let columns = Columns::from(
            &query_result.sorbet_schema,
            &query_result.original_schema,
            &self.column_blueprint_fn,
        );

        let display_record_batches: Result<Vec<_>, _> = query_result
            .sorbet_batches
            .iter()
            .map(|record_batch| {
                DisplayRecordBatch::try_new(itertools::izip!(
                    query_result.sorbet_schema.columns.iter().map(|x| x.into()),
                    columns.iter().map(|column| &column.blueprint),
                    record_batch.columns().iter().map(Arc::clone)
                ))
            })
            .try_collect();

        let display_record_batches = match display_record_batches {
            Ok(display_record_batches) => display_record_batches,
            Err(err) => {
                //TODO(ab): better error handling?
                ui.error_label(err.to_string());
                return TableUiOutput {
                    query_data,
                    flag_column: None,
                    flag_changes: Vec::new(),
                };
            }
        };

        let mut table_config =
            UiTableConfig::from_egui_state_merged_with_data_columns(ui.ctx(), static_id, &columns);

        let table_cards_and_blueprints_enabled =
            ctx.app_options.experimental.table_cards_and_blueprints;

        let blueprint_db = table_blueprints
            .active_id(&self.table_ref)
            .and_then(|store_id| ctx.storage_context.bundle.get(store_id));
        let blueprint = blueprint_db
            .map(TableBlueprint::load)
            .unwrap_or_default()
            .apply_heuristics(
                &query_result.original_schema,
                &columns,
                &display_record_batches,
                &table_config,
                self.table_ref.url().as_ref().map(|uri| uri.origin()),
            );

        let view_renderer = if table_cards_and_blueprints_enabled {
            blueprint_db.and_then(crate::preview_renderer::RecordingPreviewRenderer::from_blueprint)
        } else {
            None
        };

        let view_mode_id = session_id.with("view_mode");
        let mut view_mode = if table_cards_and_blueprints_enabled {
            ui.ctx()
                .data(|d| d.get_temp::<TableViewMode>(view_mode_id))
                .unwrap_or_default()
        } else {
            TableViewMode::Table
        };

        toolbar_ui(
            ui,
            ctx,
            &columns,
            &mut table_config,
            self.title.as_deref(),
            self.table_ref.url().map(|url| url.to_string()).as_deref(),
            should_show_loading_indicator,
            if table_cards_and_blueprints_enabled {
                Some(&mut view_mode)
            } else {
                None
            },
        );

        filter_state.filter_bar_ui(
            ui,
            ctx.app_options.timestamp_format,
            &mut query_data.column_filters,
        );

        let table_style = re_ui::TableStyle::Spacious;

        let mut row_height = ctx.tokens().table_row_height(table_style);

        // If the first column is a blob, we treat it as a thumbnail and increase the row height.
        // TODO(lucas): This is a band-aid fix and should be replaced with proper table blueprint
        let first_column = table_config
            .visible_column_names()
            .next()
            .and_then(|name| columns.find_by_physical_name(name))
            .and_then(|(index, _)| display_record_batches.first()?.columns().get(index));
        if let Some(DisplayColumn::Component(component)) = first_column
            && component.is_image()
        {
            row_height *= 3.0;
        }

        let migrated_fields = query_result
            .sorbet_schema
            .columns
            .arrow_fields(re_sorbet::BatchType::Dataframe);

        let show_segment_previews = view_mode == TableViewMode::Table
            && view_renderer.is_some()
            && blueprint
                .segment_preview_column
                .as_ref()
                .is_some_and(|name| columns.index_by_physical_name(name).is_some());

        let entry_uri = if let Some(re_uri::RedapUri::Entry(entry_uri)) = self.table_ref.url() {
            Some(entry_uri)
        } else {
            None
        };
        let flag_column = Self::resolve_flag_column(
            &blueprint,
            &columns,
            &query_result.original_schema,
            entry_uri.as_ref(),
            ctx.connection_registry,
        );
        if show_segment_previews {
            // Ensure rows are tall enough for the segment preview.
            row_height = row_height.max(SEGMENT_PREVIEW_SIZE);
        }

        let visible_columns = table_config.visible_columns().count();
        let total_columns = columns.columns.len();

        let action = Self::bottom_bar_ui(
            ui,
            ctx,
            session_id,
            num_rows,
            visible_columns,
            total_columns,
            queried_at,
        );

        match action {
            Some(BottomBarAction::Refresh) => {
                Self::refresh(
                    runtime,
                    ui.ctx().clone(),
                    Arc::clone(&self.session_ctx),
                    self.datafusion_table_ref.clone(),
                );
            }
            None => {}
        }

        let mut flag_changes = Vec::new();
        match view_mode {
            TableViewMode::Table => {
                let num_preview_views = show_segment_previews
                    .then(|| view_renderer.as_ref().map_or(1, |r| r.num_views()));

                let mut table_delegate = DataFusionTableDelegate {
                    session_id,
                    ctx,
                    table_style,
                    query_result,
                    migrated_fields: &migrated_fields,
                    display_record_batches: &display_record_batches,
                    columns: &columns,
                    blueprint: &blueprint,
                    query_data: &mut query_data,
                    filter_state: &mut filter_state,
                    row_height,
                    num_preview_views,
                    view_renderer,
                    view_states,
                };

                let visible_columns = table_config
                    .visible_columns()
                    .filter_map(|config| columns.index_by_physical_name(config.column_name()))
                    .map(|index| (egui::Id::new(columns.columns[index].physical_name()), index));
                ReTable::new(
                    ui.ctx(),
                    session_id,
                    &mut table_delegate,
                    visible_columns,
                    num_rows,
                )
                .preview_column(num_preview_views)
                .show(ui);
            }
            TableViewMode::Cards => {
                flag_changes = crate::cards_view::cards_ui(
                    ctx,
                    ui,
                    &columns,
                    &display_record_batches,
                    &table_config,
                    &blueprint,
                    view_renderer.as_ref(),
                    view_states,
                    num_rows,
                    flag_column.is_some(),
                );
            }
        }

        table_config.store(ui.ctx());
        filter_state.store(ui.ctx(), session_id);
        ui.ctx()
            .data_mut(|d| d.insert_temp(view_mode_id, view_mode));

        TableUiOutput {
            query_data,
            flag_column,
            flag_changes,
        }
    }

    fn bottom_bar_ui(
        ui: &mut Ui,
        ctx: &AppContext<'_>,
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
        Panel::bottom(session_id.with("bottom_bar"))
            .frame(frame)
            .show_separator_line(false)
            .show(ui, |ui| {
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
                            let refresh_tooltip = match re_ui::TableCommandKind::Refresh
                                .formatted_kb_shortcut(ui.ctx())
                            {
                                Some(shortcut) => format!("Refresh table ({shortcut})"),
                                None => "Refresh table".to_owned(),
                            };
                            if ui
                                .small_icon_button(&icons::RESET, "Refresh table")
                                .on_hover_text(refresh_tooltip)
                                .clicked()
                            {
                                action = Some(BottomBarAction::Refresh);
                            }

                            re_ui::time::short_duration_ui(
                                ui,
                                queried_at,
                                ctx.app_options.timestamp_format,
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
    table_ref: &DataFusionTableReference,
) -> Id {
    egui::Id::new((session_ctx.session_id(), table_ref))
}

/// The row on top of the table, with the title, if any, and the controls for how it is displayed.
///
/// Callers that show the table's name themselves pass no title, and still get the controls.
fn toolbar_ui(
    ui: &mut egui::Ui,
    ctx: &AppContext<'_>,
    columns: &Columns<'_>,
    table_config: &mut UiTableConfig,
    title: Option<&str>,
    url: Option<&str>,
    should_show_loading_indicator: bool,
    view_mode: Option<&mut TableViewMode>,
) {
    // A row of small buttons needs less room around it than a heading does.
    let inner_margin = if title.is_some() {
        Margin {
            top: 16,
            bottom: 12,
            left: 16,
            right: 16,
        }
    } else {
        Margin::symmetric(16, 8)
    };

    Frame::new().inner_margin(inner_margin).show(ui, |ui| {
        egui::Sides::new().show(
            ui,
            |ui| {
                if let Some(title) = title {
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
                }

                if should_show_loading_indicator {
                    ui.loading_indicator("Fetching table data");
                }
            },
            |ui| {
                ui.horizontal_centered(|ui| {
                    if let Some(view_mode) = view_mode {
                        ui.selectable_toggle(|ui| {
                            ui.icon_selectable_value(
                                &icons::TABLE_ROW_VIEW,
                                "Table view",
                                view_mode,
                                TableViewMode::Table,
                            );
                            ui.icon_selectable_value(
                                &icons::TABLE_GRID_VIEW,
                                "Cards view",
                                view_mode,
                                TableViewMode::Cards,
                            );
                        });
                    }

                    table_config.button_ui(ui, columns);
                });
            },
        );
    });
}

/// Find the record batch and local row index for a global row index.
pub fn find_row_batch(
    batches: &[DisplayRecordBatch],
    mut row_index: usize,
) -> Option<(&DisplayRecordBatch, usize)> {
    for batch in batches {
        let row_count = batch.num_rows();
        if row_index < row_count {
            return Some((batch, row_index));
        }
        row_index -= row_count;
    }
    None
}

pub fn value_at(
    columns: &Columns<'_>,
    display_record_batches: &[DisplayRecordBatch],
    row: u64,
    column_name: &ColumnName,
) -> Option<arrow::array::ArrayRef> {
    let (display_record_batch, local_row_index) =
        find_row_batch(display_record_batches, row as usize)?;
    let column_index = columns.index_by_physical_name(column_name)?;
    let column = display_record_batch.columns().get(column_index)?;

    match column {
        DisplayColumn::RowId { .. } | DisplayColumn::Timeline { .. } => None,
        DisplayColumn::Component(col) => col.row_value_at(local_row_index),
    }
}

/// Extract a string value from a named column at the given row.
pub fn string_value_at(
    columns: &Columns<'_>,
    display_record_batches: &[DisplayRecordBatch],
    row: u64,
    column_name: &ColumnName,
) -> Option<String> {
    let data = value_at(columns, display_record_batches, row, column_name)?;
    let string_array = data.downcast_array_ref::<arrow::array::StringArray>()?;
    if string_array.is_empty() {
        return None;
    }
    Some(string_array.value(0).to_owned())
}

/// Resolve the recording for a row's segment URI, triggering async loading if needed.
///
/// Shared between the regular table and card layouts.
pub fn resolve_recording_for_row<'a>(
    ctx: &'a AppContext<'a>,
    segment_preview_column: &ColumnName,
    columns: &Columns<'_>,
    display_record_batches: &[DisplayRecordBatch],
    row_idx: u64,
    already_requested_uris: &mut ahash::HashSet<re_uri::DatasetUri>,
) -> Option<PreviewRecording<'a>> {
    let uri_str = string_value_at(
        columns,
        display_record_batches,
        row_idx,
        segment_preview_column,
    )?;

    // A preview cell has to name the segment to load.
    let uri = uri_str
        .parse::<re_uri::DatasetUri>()
        .ok()
        .filter(|uri| uri.segment_id.is_some())?;

    if let Some(recording) = ctx.storage_context.hub.find_recording_by_uri(&uri) {
        // Keep this recording in the preview prefetch set so chunks get streamed in.
        // Without this, the recording sits in the hub with no active query pressure
        // and the chunk prioritizer never schedules it for download.
        ctx.storage_context.hub.mark_preview(recording.store_id());
        return Some(PreviewRecording::Resolved(recording));
    }

    let uri = uri.without_fragment();

    // Not loaded yet — request loading if we haven't already.
    if already_requested_uris.insert(uri.clone()) {
        ctx.command_sender
            .send_system(SystemCommand::LoadDataSource(
                re_data_source::LogDataSource::RedapDatasetSegment {
                    uri: uri.clone(),
                    open_behavior: re_data_source::RecordingOpenBehavior::Background,
                },
            ));
    }

    Some(PreviewRecording::Unresolved(uri))
}

fn table_index_column(schema: &arrow::datatypes::Schema) -> Option<(usize, &Arc<Field>)> {
    schema.fields.iter().enumerate().find(|(_, field)| {
        field
            .metadata()
            .get(re_sorbet::metadata::SORBET_IS_TABLE_INDEX)
            .map(String::as_str)
            == Some("true")
    })
}

/// Asynchronously write flag changes back to the remote server.
///
/// Constructs a minimal `RecordBatch` with the table index column + flag column for each
/// changed row and sends it via `WriteTable` with `Update` semantics: rows are matched on
/// the table index and only their flag column is updated. Because every changed row already
/// exists in the table, the subset schema (index + flag) and `Update`'s drop-unmatched
/// behavior are exactly what we want — we never want to insert new rows here.
///
/// This is fire-and-forget: errors are logged but don't block the UI.
fn upsert_flag_changes(
    ctx: &AppContext<'_>,
    runtime: &AsyncRuntimeHandle,
    remote: &re_uri::EntryUri,
    results: &crate::datafusion_adapter::DataFusionQueryResult,
    flag_column: &ResolvedFlagColumn,
    changes: &[crate::cards_view::FlagChangeEvent],
) {
    // Collect index + flag values for each changed row.
    let mut index_arrays = Vec::new();
    let mut flag_values = Vec::new();
    for change in changes {
        if let Some((batch, row_offset)) = results.find_row_batch(change.row) {
            let Some((index_col_idx, _)) = table_index_column(batch.schema_ref()) else {
                continue;
            };

            index_arrays.push(batch.column(index_col_idx).slice(row_offset, 1));
            flag_values.push(change.new_value);
        }
    }
    if index_arrays.is_empty() {
        return;
    }

    // Build a minimal upsert record batch: [index_column, flag_column].
    let build_upsert_batch = || -> Option<arrow::array::RecordBatch> {
        let index_refs: Vec<_> = index_arrays.iter().map(|a| a.as_ref()).collect();
        let index_array = re_arrow_util::concat_arrays(&index_refs).ok()?;
        let flag_array = Arc::new(BooleanArray::from(flag_values));

        let schema = arrow::datatypes::Schema::new_with_metadata(
            vec![
                flag_column.index_field.clone(),
                flag_column.flag_field.clone(),
            ],
            Default::default(),
        );

        let num_rows = index_array.len();
        arrow::array::RecordBatch::try_new_with_options(
            Arc::new(schema),
            vec![index_array, flag_array],
            &arrow::array::RecordBatchOptions::new().with_row_count(Some(num_rows)),
        )
        .ok()
    };
    let Some(upsert_batch) = build_upsert_batch() else {
        re_log::warn_once!("Failed to build upsert RecordBatch for flag changes");
        return;
    };

    let connection = ctx
        .connection_registry
        .connection_handle(remote.origin.clone());
    let entry_id = remote.entry_id;
    runtime.spawn_future(async move {
        let result = async {
            let mut client = connection.client().await?;
            client
                .write_table(
                    futures::stream::once(async { upsert_batch }),
                    entry_id,
                    // `Update`: match existing rows on the table index and update only the
                    // flag column. Unmatched source rows are dropped, which is correct here
                    // since we only ever edit flags on rows that already exist.
                    ext::TableInsertMode::Update,
                )
                .await
        }
        .await;

        if let Err(err) = result {
            re_log::warn_once!("Failed to upsert flag changes: {err}");
        } else {
            re_log::debug!("Successfully upserted flag changes");
        }
    });
}

enum BottomBarAction {
    Refresh,
}

struct DataFusionTableDelegate<'a> {
    session_id: Id,
    ctx: &'a AppContext<'a>,
    table_style: re_ui::TableStyle,
    query_result: &'a DataFusionQueryResult,
    migrated_fields: &'a Vec<Field>,
    display_record_batches: &'a Vec<DisplayRecordBatch>,
    columns: &'a Columns<'a>,
    blueprint: &'a TableBlueprint,
    query_data: &'a mut DataFusionQueryData,
    filter_state: &'a mut FilterState,
    row_height: f32,

    /// Number of preview views to show per row, if previews are enabled.
    num_preview_views: Option<usize>,

    /// Renderer for the segment preview column (column 0 in the delegate's column space).
    /// `None` when no table blueprint is registered for this table.
    view_renderer: Option<crate::preview_renderer::RecordingPreviewRenderer<'a>>,

    /// Shared view states for segment preview views, persisted across frames.
    view_states: &'a mut re_viewer_context::ViewStates,
}

impl DataFusionTableDelegate<'_> {
    fn segment_link_for_row(&self, row: u64, spec: &SegmentLinksSpec) -> Option<String> {
        string_value_at(
            self.columns,
            self.display_record_batches,
            row,
            &spec.column_name,
        )
    }

    pub fn row_context_menu(&self, ui: &Ui, _row_number: u64) {
        let has_context_menu = self.query_data.segment_links.is_some();
        if !has_context_menu {
            return;
        }

        ui.response().container_context_menu(|ui| {
            let selection = TableSelectionState::load(ui.ctx(), self.session_id);

            // re_table will ensure that the right-clicked row is always selected.
            let selected_rows = selection.selected_rows;

            if let Some(segment_links_spec) = &self.query_data.segment_links {
                let label = format!("Open {}", format_plural_s(selected_rows.len(), "segment"));
                let response =
                    ui.add(icons::OPEN_RECORDING.as_button_with_label(ui.tokens(), label));

                let open = |new_tab| {
                    // Let's open the recordings in order
                    for row in selected_rows.iter().copied().sorted() {
                        if let Some(segment_link) =
                            self.segment_link_for_row(row, segment_links_spec)
                        {
                            ui.open_url(OpenUrl {
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
        let num_extra = self.num_preview_views.is_some() as usize;
        if cell.group_index < num_extra {
            // Segment preview header -- empty.
            return;
        }

        let tokens = ui.tokens();
        let table_style = self.table_style;
        let col_index = cell.group_index - num_extra;
        if let Some(column) = self.columns.columns.get(col_index) {
            let column_field = &self.query_result.original_schema.fields[col_index];
            let column_physical_name = column.physical_name();
            let column_display_name = column.display_name();

            let current_sort_direction = self.query_data.sort_by.as_ref().and_then(|sort_by| {
                (&sort_by.column_name == column_physical_name).then_some(sort_by.direction)
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

                        if let Some(dir_icon) =
                            current_sort_direction.as_ref().map(SortDirection::icon)
                        {
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
                                let already_sorted = Some(sort_direction) == current_sort_direction;

                                if ui
                                    .add_enabled_ui(!already_sorted, |ui| {
                                        sort_direction.menu_item_ui(ui)
                                    })
                                    .inner
                                    .clicked()
                                {
                                    self.query_data.sort_by = Some(column.sort_by(sort_direction));
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
        // Column 1 is reserved for the preview column if enabled.
        let has_preview_column = self.num_preview_views.is_some();
        if cell.col_nr == 0
            && has_preview_column
            && let Some(renderer) = &self.view_renderer
            && let Some(segment_preview_column) = &self.blueprint.segment_preview_column
        {
            let preview_state = self.view_states.preview_state.get_or_insert_default();
            let recording = resolve_recording_for_row(
                self.ctx,
                segment_preview_column,
                self.columns,
                self.display_record_batches,
                cell.row_nr,
                &mut preview_state.requested_uris,
            );

            let row_hovered = TableSelectionState::load(ui.ctx(), self.session_id).hovered_row
                == Some(cell.row_nr);

            renderer.show_preview(
                self.ctx,
                ui,
                cell.row_nr,
                row_hovered,
                recording,
                self.view_states,
            );
        } else {
            let col_index = cell.col_nr.saturating_sub(has_preview_column as usize);

            if let Some((display_record_batch, batch_index)) =
                find_row_batch(self.display_record_batches, cell.row_nr as usize)
            {
                let column = &display_record_batch.columns()[col_index];

                column.data_ui(self.ctx, ui, batch_index, None, UiLayout::List, None, false);
            }
        }
    }

    fn row_ui(&mut self, ui: &mut Ui, row_nr: u64) {
        self.row_context_menu(ui, row_nr);
    }

    fn default_row_height(&self) -> f32 {
        self.row_height
    }
}
