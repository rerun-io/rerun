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
use re_sdk_types::blueprint::components::{ColumnName, TableCellKind, TableLayoutKind};
use re_sorbet::{ColumnDescriptorRef, SorbetSchema};
use re_ui::egui_ext::response_ext::ResponseExt as _;
use re_ui::menu::menu_style;
use re_ui::{UiExt as _, UiLayout, icons};
use re_viewer_context::{
    AppBlueprintCtx, AppContext, BlueprintContext as _, MaybeMutRef, SystemCommand,
    SystemCommandSender as _, TableReference,
};

use crate::DisplayRecordBatch;
use crate::blueprint::{TableBlueprint, TableColumn};
use crate::cards_view::FlagChangeEvent;
use crate::column_sorting::{SortBy, SortDirection};
use crate::datafusion_adapter::{
    DataFusionAdapter, DataFusionQueryData, DataFusionQueryResult, EntryLinksSpec, SegmentLinksSpec,
};
use crate::display_record_batch::DisplayColumn;
use crate::filters::{ColumnFilter, FilterState};
use crate::header_tooltip::column_header_tooltip_ui;
use crate::re_table::{PreviewColumn, ReTable};
use crate::re_table_utils::columns_edit_menu_ui;
use crate::table_selection::TableSelectionState;
use crate::{StreamingCacheTableProvider, TableBlueprints};

/// Minimum row height (in points) when segment preview views are shown.
const SEGMENT_PREVIEW_SIZE: f32 = 200.0;

/// Output produced by [`DataFusionTableWidget::table_ui`].
struct TableUiOutput {
    /// The (potentially modified) query state.
    query_data: DataFusionQueryData,

    /// Resolved source information for flag mutation and write-back.
    flag_columns: Vec<ResolvedFlagColumn>,

    /// Flag changes from either layout.
    flag_changes: Vec<FlagChangeEvent>,
}

pub struct ResolvedFlagColumn {
    pub physical_name: ColumnName,
    flag_field: Arc<Field>,
    index_field: Arc<Field>,
}

/// Represents a single column in the data, including its original Arrow field name and descriptor.
///
/// This is related, but independent of its blueprint representation [`TableColumn`].
pub struct DataColumn<'a> {
    /// The original Arrow field name used by DataFusion to identify the column.
    ///
    /// [`ColumnName`] always refers to this original name, never a name produced by Sorbet
    /// migration.
    physical_name: ColumnName,

    /// Reference to the descriptor of this column.
    pub desc: ColumnDescriptorRef<'a>,
}

impl DataColumn<'_> {
    /// Returns the original Arrow field name used by DataFusion.
    pub fn physical_name(&self) -> &ColumnName {
        &self.physical_name
    }
}

/// Keep track of a [`re_sorbet::SorbetBatch`]'s columns, along with their original order.
pub struct DataColumns<'a> {
    pub columns: Vec<DataColumn<'a>>,
}

impl<'a> DataColumns<'a> {
    fn from(sorbet_schema: &'a SorbetSchema, original_schema: &arrow::datatypes::Schema) -> Self {
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

                DataColumn {
                    physical_name,
                    desc,
                }
            })
            .collect();

        Self { columns }
    }
}

impl DataColumns<'_> {
    pub fn iter(&self) -> impl Iterator<Item = &DataColumn<'_>> + use<'_> {
        self.columns.iter()
    }

    /// Find a column by its physical name.
    pub fn find_by_physical_name(&self, name: &ColumnName) -> Option<(usize, &DataColumn<'_>)> {
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

pub type TableColumnHeuristic<'a> = Box<
    dyn for<'column> Fn(&ColumnDescriptorRef<'column>, TableColumn<'column>) -> TableColumn<'column>
        + 'a,
>;

type ToolbarSummaryFn<'a> = Box<dyn Fn(&mut Ui) + 'a>;

pub struct DataFusionTableWidget<'a> {
    session_ctx: Arc<SessionContext>,

    /// Stable identity used by table-scoped actions.
    table_ref: TableReference,

    datafusion_table_ref: DataFusionTableReference,

    /// If provided, the toolbar on top of the table shows this as its title.
    title: Option<String>,

    /// If provided, the toolbar on top of the table shows this at its left end.
    toolbar_summary_fn: Option<ToolbarSummaryFn<'a>>,

    /// Runtime defaults applied after stored table blueprint values are resolved.
    additional_column_heuristics: TableColumnHeuristic<'a>,

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

    /// When the client last queried the table, or `None` if it hasn't queried it yet.
    ///
    /// This is the age of what the user is looking at, not of the table on the server.
    /// [`Self::refresh`] resets it.
    pub fn queried_at(
        ui: &egui::Ui,
        session_ctx: &SessionContext,
        table_ref: impl Into<DataFusionTableReference>,
    ) -> Option<Timestamp> {
        let id = id_from_session_context_and_table(session_ctx, &table_ref.into());
        ui.data(|data| data.get_temp::<DataFusionAdapter>(id))
            .map(|adapter| adapter.queried_at)
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
            toolbar_summary_fn: None,
            additional_column_heuristics: Box::new(|_, column| column),
            initial_query_data: Default::default(),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());

        self
    }

    /// Shows the caller's own UI at the left end of the toolbar, before the loading indicator.
    ///
    /// For what the caller knows about the table and the table doesn't, e.g. how many segments the
    /// dataset holds.
    pub fn toolbar_summary(mut self, summary_ui: impl Fn(&mut Ui) + 'a) -> Self {
        self.toolbar_summary_fn = Some(Box::new(summary_ui));

        self
    }

    pub fn additional_column_heuristics(
        mut self,
        heuristic: impl for<'column> Fn(
            &ColumnDescriptorRef<'column>,
            TableColumn<'column>,
        ) -> TableColumn<'column>
        + 'a,
    ) -> Self {
        self.additional_column_heuristics = Box::new(heuristic);
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

    fn resolve_flag_columns(
        blueprint: &TableBlueprint<'_>,
        layout_kind: TableLayoutKind,
        columns: &DataColumns<'_>,
        original_schema: &arrow::datatypes::Schema,
        remote_table: Option<&re_uri::EntryUri>,
        connection_registry: &re_redap_client::ConnectionRegistryHandle,
    ) -> Vec<ResolvedFlagColumn> {
        blueprint
            .iter_columns(layout_kind)
            .filter(|column| {
                column.configured_cell_kind() == TableCellKind::Flag && column.editable()
            })
            .filter_map(|flag_column| {
                let name = flag_column.physical_name();
                if columns.find_by_physical_name(name).is_none() {
                    re_log::warn_once!("Flag column {name:?} does not exist in the table");
                    return None;
                }
                Self::resolve_flag_column_source(
                    name,
                    original_schema,
                    remote_table,
                    connection_registry,
                )
            })
            .collect()
    }

    fn resolve_flag_column_source(
        name: &ColumnName,
        original_schema: &arrow::datatypes::Schema,
        remote_table: Option<&re_uri::EntryUri>,
        connection_registry: &re_redap_client::ConnectionRegistryHandle,
    ) -> Option<ResolvedFlagColumn> {
        let Some(remote_table) = remote_table else {
            re_log::warn_once!(
                "Flag column {name:?} is not editable because the table is not a writable remote table"
            );
            return None;
        };
        let Some(flag_field) = original_schema
            .fields()
            .iter()
            .find(|field| field.name() == name.as_str())
        else {
            re_log::warn_once!("Flag column {name:?} is missing from the original schema");
            return None;
        };
        if flag_field.data_type() != &arrow::datatypes::DataType::Boolean {
            re_log::warn_once!("Flag column {name:?} is not boolean");
            return None;
        }
        let Some((_, index_field)) = table_index_column(original_schema) else {
            re_log::warn_once!(
                "Flag column {name:?} is not editable because the table has no rerun:is_table_index column"
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
            physical_name: name.clone(),
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
        let table_exist_result = self
            .session_ctx
            .table_exist(self.datafusion_table_ref.clone());

        if matches!(table_exist_result, Err(_) | Ok(false)) {
            let error_string = table_exist_result
                .map_err(|err| re_error::format_ref(&err))
                .err();
            ui.loading_screen(
                "Loading table:",
                self.display_name().as_ref(),
                error_string.as_deref(),
                None,
            );
            return if let Some(error) = error_string {
                TableStatus::Error(error)
            } else {
                TableStatus::InitialLoading
            };
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
                ui.loading_screen("Loading table:", self.display_name().as_ref(), None, None);
                return TableStatus::InitialLoading;
            }
        };

        let TableUiOutput {
            query_data,
            flag_columns,
            flag_changes,
        } = self.table_ui(
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

        if !flag_changes.is_empty() {
            table_state.apply_flag_changes(ui, &flag_changes);

            if let Some(re_uri::RedapUri::Entry(entry_uri)) = &self.table_ref.url()
                && let Some(Ok(results)) = &table_state.results
            {
                for change in flag_changes {
                    let Some(flag_column) = flag_columns
                        .iter()
                        .find(|column| column.physical_name == change.physical_column)
                    else {
                        re_log::warn_once!(
                            "Flag column {:?} is not editable",
                            change.physical_column
                        );
                        continue;
                    };
                    upsert_flag_change(app_ctx, runtime, entry_uri, results, flag_column, &change);
                }
            }

            // Invalidate the streaming cache so the next re-query (e.g. filter/sort change)
            // fetches fresh data from the server (which now has the upserted flags).
            let session_ctx = Arc::clone(&self.session_ctx);
            let table_ref = self.datafusion_table_ref.clone();
            runtime.spawn_future(async move {
                Self::invalidate_streaming_cache(&session_ctx, &table_ref).await;
            });
        }

        if table_state.query_data() != &query_data {
            table_state.update_query(runtime, ui, query_data);
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

        let data_columns =
            DataColumns::from(&query_result.sorbet_schema, &query_result.original_schema);

        let blueprint_ctx = table_blueprints.blueprint_context_for(ctx, &self.table_ref);
        let blueprint = TableBlueprint::load_and_resolve(
            &blueprint_ctx,
            &data_columns,
            &self.additional_column_heuristics,
        );

        let mut layout_kind = blueprint.layout();
        let card_layout_available = blueprint.card_layout.is_some();

        let display_record_batches: Result<Vec<_>, _> = query_result
            .sorbet_batches
            .iter()
            .map(|record_batch| {
                // The order in which we query should be the same order in which we receive, so we should be able to zip things up just fine.
                // TODO(andreas): seems brittle with sorbet migrations?
                DisplayRecordBatch::try_new(itertools::izip!(
                    query_result.sorbet_schema.columns.iter().map(|x| x.into()),
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
                    flag_columns: Vec::new(),
                    flag_changes: Vec::new(),
                };
            }
        };

        let blueprint_columns = blueprint.iter_columns(layout_kind);
        let layout_kind_ref = if card_layout_available {
            MaybeMutRef::MutRef(&mut layout_kind)
        } else {
            MaybeMutRef::Ref(&layout_kind)
        };
        toolbar_ui(
            ui,
            ctx,
            &blueprint_ctx,
            blueprint_columns,
            self.title.as_deref(),
            self.toolbar_summary_fn.as_deref(),
            self.table_ref.url().map(|url| url.to_string()).as_deref(),
            should_show_loading_indicator,
            layout_kind_ref,
        );
        if layout_kind != blueprint.layout() {
            TableBlueprint::save_layout(&blueprint_ctx, layout_kind);
        }

        // Under a tab bar the toolbar's own bottom margin is the whole gap to what follows, so
        // `item_spacing` must not add to it. A titled table keeps the spacing it had.
        if self.title.is_none() {
            ui.spacing_mut().item_spacing.y = 0.0;
        }

        filter_state.filter_bar_ui(
            ui,
            ctx.app_options.timestamp_format,
            &mut query_data.column_filters,
        );

        let blueprint_db = blueprint_ctx.current_blueprint();
        let view_renderers = blueprint
            .iter_visible_columns(layout_kind)
            .filter(|column| column.configured_cell_kind() == TableCellKind::Preview)
            .filter_map(|column| {
                let data_column_index =
                    data_columns.index_by_physical_name(column.physical_name())?;
                crate::preview_renderer::RecordingPreviewRenderer::from_previews_config(
                    blueprint_db,
                    column.physical_name().clone(),
                    data_column_index,
                    column.preview_views_blueprint_paths(),
                    &blueprint.previews_config,
                )
            })
            .collect::<Vec<_>>();

        let migrated_fields = query_result
            .sorbet_schema
            .columns
            .arrow_fields(re_sorbet::BatchType::Dataframe);

        let potentially_writable_remote_table = self.table_ref.url().and_then(|uri| match uri {
            re_uri::RedapUri::Entry(entry) => Some(entry),
            _ => None,
        });
        let flag_columns = if should_show_loading_indicator {
            Vec::new()
        } else {
            Self::resolve_flag_columns(
                &blueprint,
                layout_kind,
                &data_columns,
                &query_result.original_schema,
                potentially_writable_remote_table.as_ref(),
                ctx.connection_registry,
            )
        };

        let num_visible_columns = blueprint.iter_visible_columns(layout_kind).count();
        let num_total_columns = data_columns.columns.len();

        let action = Self::bottom_bar_ui(
            ui,
            ctx,
            session_id,
            num_rows,
            num_visible_columns,
            num_total_columns,
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
        match layout_kind {
            TableLayoutKind::Table => {
                let table_style = re_ui::TableStyle::Spacious;

                let mut row_height = ctx.tokens().table_row_height(table_style);

                let first_column_kind =
                    blueprint
                        .iter_visible_columns(layout_kind)
                        .next()
                        .map(|column| {
                            let display_column = data_columns
                                .index_by_physical_name(column.physical_name())
                                .and_then(|index| {
                                    display_record_batches.first()?.columns().get(index)
                                });
                            // Probe row 0 to determine the cell kind.
                            // TODO(andreas): this is brittle! What if the first one is null and the next is a thumbnail.
                            column.value_resolved_cell_kind(display_column, 0)
                        });
                if first_column_kind == Some(TableCellKind::Thumbnail) {
                    row_height *= 3.0;
                }

                let preview_columns = view_renderers
                    .iter()
                    .map(|renderer| PreviewColumn {
                        data_column_index: renderer.data_column_index(),
                        num_views: renderer.num_views(),
                        preview_height: SEGMENT_PREVIEW_SIZE,
                    })
                    .collect::<Vec<_>>();
                if !preview_columns.is_empty() {
                    row_height = row_height.max(
                        SEGMENT_PREVIEW_SIZE
                            + ctx.tokens().table_row_height(table_style)
                            + ui.spacing().item_spacing.y,
                    );
                }
                let visible_columns = blueprint
                    .iter_visible_columns(layout_kind)
                    .filter_map(|column| {
                        data_columns.index_by_physical_name(column.physical_name())
                    })
                    .map(|index| {
                        (
                            egui::Id::new(data_columns.columns[index].physical_name()),
                            index,
                        )
                    });

                let mut table_delegate = DataFusionTableDelegate {
                    session_id,
                    ctx,
                    table_style,
                    query_result,
                    migrated_fields: &migrated_fields,
                    display_record_batches: &display_record_batches,
                    columns: &data_columns,
                    blueprint: &blueprint,
                    query_data: &mut query_data,
                    filter_state: &mut filter_state,
                    row_height,
                    view_renderers,
                    view_states,
                    flag_columns: &flag_columns,
                    flag_changes: &mut flag_changes,
                };

                // The table paints its own background rather than letting the page show through,
                // so it matches the cards on the other tabs.
                Frame::new()
                    .fill(ui.tokens().extreme_bg_color)
                    .show(ui, |ui| {
                        ReTable::new(
                            ui.ctx(),
                            session_id,
                            &mut table_delegate,
                            visible_columns,
                            num_rows,
                        )
                        .with_preview_columns(preview_columns)
                        .show(ui);
                    });
            }
            TableLayoutKind::Cards => {
                if let Some(card_layout) = blueprint.card_layout {
                    flag_changes = crate::cards_view::cards_ui(
                        ctx,
                        ui,
                        &data_columns,
                        &display_record_batches,
                        &card_layout,
                        &view_renderers,
                        view_states,
                        num_rows,
                        &flag_columns,
                    );
                } else {
                    // Should never happen.
                    ui.error_label("No card layout defined in the table blueprint");
                }
            }
        }

        filter_state.store(ui.ctx(), session_id);

        TableUiOutput {
            query_data,
            flag_columns,
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

/// The row above the table, with an optional title and the display controls.
fn toolbar_ui<'a>(
    ui: &mut egui::Ui,
    ctx: &AppContext<'_>,
    blueprint_ctx: &AppBlueprintCtx<'_>,
    blueprint_columns: impl Iterator<Item = &'a TableColumn<'a>>,
    title: Option<&str>,
    summary_ui: Option<&dyn Fn(&mut Ui)>,
    url: Option<&str>,
    should_show_loading_indicator: bool,
    mut layout_kind: MaybeMutRef<'_, TableLayoutKind>,
) {
    // A row of small buttons needs less room around it than a heading does. Without a title this
    // is the row under a tab bar, so it uses `TAB_TOOLBAR_MARGIN_Y` like every other tab.
    let inner_margin = if title.is_some() {
        Margin {
            top: 16,
            bottom: 12,
            left: 16,
            right: 16,
        }
    } else {
        Margin::symmetric(16, re_ui::TAB_TOOLBAR_MARGIN_Y as i8)
    };

    // Fixed, so the row is the same height whatever it holds.
    let row_height = title.is_none().then_some(re_ui::TAB_TOOLBAR_HEIGHT);

    Frame::new().inner_margin(inner_margin).show(ui, |ui| {
        egui::Sides::new().show(
            ui,
            |ui| {
                if let Some(row_height) = row_height {
                    ui.set_height(row_height);
                }

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

                if let Some(summary_ui) = summary_ui {
                    summary_ui(ui);
                }

                if should_show_loading_indicator {
                    ui.loading_indicator("Fetching table data");
                }
            },
            |ui| {
                if let Some(row_height) = row_height {
                    ui.set_height(row_height);
                }

                ui.horizontal_centered(|ui| {
                    if let Some(layout_kind) = layout_kind.as_mut() {
                        ui.selectable_toggle(|ui| {
                            ui.icon_selectable_value(
                                &icons::TABLE_ROW_VIEW,
                                "Table view",
                                layout_kind,
                                TableLayoutKind::Table,
                            );
                            ui.icon_selectable_value(
                                &icons::TABLE_GRID_VIEW,
                                "Cards view",
                                layout_kind,
                                TableLayoutKind::Cards,
                            );
                        });
                    }

                    columns_edit_menu_ui(ui, blueprint_ctx, *layout_kind, blueprint_columns);
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
    columns: &DataColumns<'_>,
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
    columns: &DataColumns<'_>,
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

fn table_index_column(schema: &arrow::datatypes::Schema) -> Option<(usize, &Arc<Field>)> {
    schema.fields.iter().enumerate().find(|(_, field)| {
        field
            .metadata()
            .get(re_sorbet::metadata::SORBET_IS_TABLE_INDEX)
            .map(String::as_str)
            == Some("true")
    })
}

/// Asynchronously writes a flag change back to the remote server.
///
/// The update contains the table index and flag column only, so it can update the existing row
/// without replacing its other values.
fn upsert_flag_change(
    ctx: &AppContext<'_>,
    runtime: &AsyncRuntimeHandle,
    remote: &re_uri::EntryUri,
    results: &crate::datafusion_adapter::DataFusionQueryResult,
    flag_column: &ResolvedFlagColumn,
    change: &crate::cards_view::FlagChangeEvent,
) {
    let Some((batch, row_offset)) = results.find_row_batch(change.row) else {
        return;
    };
    let Some((index_col_idx, _)) = table_index_column(batch.schema_ref()) else {
        return;
    };

    let index_array = batch.column(index_col_idx).slice(row_offset, 1);
    let flag_array = Arc::new(BooleanArray::from(vec![change.new_value]));
    let schema = arrow::datatypes::Schema::new_with_metadata(
        vec![
            flag_column.index_field.clone(),
            flag_column.flag_field.clone(),
        ],
        Default::default(),
    );
    let Ok(upsert_batch) = arrow::array::RecordBatch::try_new_with_options(
        Arc::new(schema),
        vec![index_array, flag_array],
        &arrow::array::RecordBatchOptions::new().with_row_count(Some(1)),
    ) else {
        re_log::warn_once!("Failed to build upsert RecordBatch for a flag change");
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
    columns: &'a DataColumns<'a>,
    blueprint: &'a TableBlueprint<'a>,
    query_data: &'a mut DataFusionQueryData,
    filter_state: &'a mut FilterState,
    row_height: f32,

    /// Renderers for the configured preview columns.
    view_renderers: Vec<crate::preview_renderer::RecordingPreviewRenderer<'a>>,

    /// Shared view states for segment preview views, persisted across frames.
    view_states: &'a mut re_viewer_context::ViewStates,
    flag_columns: &'a [ResolvedFlagColumn],
    flag_changes: &'a mut Vec<FlagChangeEvent>,
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
        let tokens = ui.tokens();
        let table_style = self.table_style;
        let col_index = cell.group_index;
        if let Some(data_column) = self.columns.columns.get(col_index) {
            let column_field = &self.query_result.original_schema.fields[col_index];

            let Some(column) = self
                .blueprint
                .column_by_physical_name(TableLayoutKind::Table, data_column.physical_name())
            else {
                re_log::debug_panic!(
                    "Blueprint column not found for physical name {:?}. All names should be filled out in the blueprint. This is a bug in the blueprint resolver.",
                    data_column.physical_name()
                );
                return;
            };
            if !column.is_visible(TableLayoutKind::Table) {
                return;
            }

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
                                    self.query_data.sort_by = Some(SortBy {
                                        column_name: column.physical_name().clone(),
                                        direction: sort_direction,
                                    });
                                    ui.close();
                                }
                            }

                            #[expect(clippy::collapsible_if)]
                            if let Some(column_filter) =
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
                            &data_column.desc,
                            column_field,
                            &self.migrated_fields[col_index],
                            show_extras,
                        );
                    });
                });
        }
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &CellInfo) {
        let Some((display_record_batch, batch_index)) =
            find_row_batch(self.display_record_batches, cell.row_nr as usize)
        else {
            // We don't have any data for this cell. That's weird!
            return;
        };

        let Some(column) = &display_record_batch.columns().get(cell.col_nr) else {
            return;
        };

        if let Some(renderer) = self
            .view_renderers
            .iter()
            .find(|renderer| renderer.data_column_index() == cell.col_nr)
        {
            let row_hovered = TableSelectionState::load(ui.ctx(), self.session_id).hovered_row
                == Some(cell.row_nr);

            ui.vertical(|ui| {
                // Show link alongside the preview.
                column.data_ui(
                    self.ctx,
                    ui,
                    batch_index,
                    None,
                    UiLayout::List,
                    TableCellKind::Link,
                    false,
                );

                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), SEGMENT_PREVIEW_SIZE),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        renderer.show_preview_for_row(
                            self.ctx,
                            ui,
                            cell.row_nr,
                            row_hovered,
                            self.display_record_batches,
                            self.view_states,
                        );
                    },
                );
            });
        } else {
            let Some(data_column) = &self.columns.columns.get(cell.col_nr) else {
                return;
            };
            let editable = self
                .flag_columns
                .iter()
                .any(|column| column.physical_name == *data_column.physical_name());
            let cell_kind = self
                .blueprint
                .column_by_physical_name(TableLayoutKind::Table, data_column.physical_name())
                .map_or(TableCellKind::Auto, |table_column| {
                    table_column.value_resolved_cell_kind(Some(column), batch_index)
                });

            let instance_index = None; // Only used in dataframe views.
            let edited = column.data_ui(
                self.ctx,
                ui,
                batch_index,
                instance_index,
                UiLayout::List,
                cell_kind,
                editable,
            );

            // Create an edit event if any edit occurred.
            if let Some(edited) = edited
                && let Some(edited) = edited.downcast_array_ref::<arrow::array::BooleanArray>()
                && !edited.is_empty()
                && !edited.is_null(0)
            {
                self.flag_changes.push(FlagChangeEvent {
                    row: cell.row_nr,
                    physical_column: data_column.physical_name().clone(),
                    new_value: edited.value(0),
                });
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

#[cfg(test)]
mod tests {
    use super::*;

    fn remote_table() -> re_uri::EntryUri {
        "rerun+http://localhost:1234/entry/1".parse().unwrap()
    }

    fn connection_registry() -> re_redap_client::ConnectionRegistryHandle {
        re_redap_client::ConnectionRegistry::new_without_stored_credentials()
    }

    fn index_field() -> Field {
        Field::new("id", arrow::datatypes::DataType::Int64, false).with_metadata(
            [(
                re_sorbet::metadata::SORBET_IS_TABLE_INDEX.to_owned(),
                "true".to_owned(),
            )]
            .into(),
        )
    }

    #[test]
    fn non_boolean_flag_is_rejected() {
        let schema = arrow::datatypes::Schema::new_with_metadata(
            vec![
                index_field(),
                Field::new("value", arrow::datatypes::DataType::Utf8, false),
            ],
            Default::default(),
        );
        assert!(
            DataFusionTableWidget::resolve_flag_column_source(
                &"value".into(),
                &schema,
                Some(&remote_table()),
                &connection_registry(),
            )
            .is_none()
        );
    }

    #[test]
    fn flag_without_table_index_is_rejected() {
        let schema = arrow::datatypes::Schema::new_with_metadata(
            vec![Field::new(
                "value",
                arrow::datatypes::DataType::Boolean,
                false,
            )],
            Default::default(),
        );
        assert!(
            DataFusionTableWidget::resolve_flag_column_source(
                &"value".into(),
                &schema,
                Some(&remote_table()),
                &connection_registry(),
            )
            .is_none()
        );
    }

    #[test]
    fn flag_with_table_index_is_resolved() {
        let schema = arrow::datatypes::Schema::new_with_metadata(
            vec![
                index_field(),
                Field::new("value", arrow::datatypes::DataType::Boolean, true),
            ],
            Default::default(),
        );
        assert!(
            DataFusionTableWidget::resolve_flag_column_source(
                &"value".into(),
                &schema,
                Some(&remote_table()),
                &connection_registry(),
            )
            .is_some()
        );
    }
}
