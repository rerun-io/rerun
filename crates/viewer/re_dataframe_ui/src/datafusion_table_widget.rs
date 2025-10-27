use std::iter;
use std::sync::Arc;

use arrow::datatypes::Field;
use datafusion::prelude::SessionContext;
use datafusion::sql::TableReference;
use egui::containers::menu::MenuConfig;
use egui::{
    FontSelection, Frame, Id, Margin, Rangef, RichText, TextWrapMode, TopBottomPanel, Ui,
    Widget as _, WidgetText,
};
use egui_table::{CellInfo, HeaderCellInfo};
use nohash_hasher::IntMap;

use re_format::format_uint;
use re_log_types::{EntryId, TimelineName, Timestamp};
use re_sorbet::{ColumnDescriptorRef, SorbetSchema};
use re_ui::menu::menu_style;
use re_ui::{UiExt as _, icons};
use re_viewer_context::{
    AsyncRuntimeHandle, SystemCommand, SystemCommandSender as _, ViewerContext,
};

use crate::datafusion_adapter::{DataFusionAdapter, DataFusionQueryResult};
use crate::display_record_batch::DisplayColumn;
use crate::filters::{ColumnFilter, FilterState};
use crate::header_tooltip::column_header_tooltip_ui;
use crate::table_blueprint::{
    ColumnBlueprint, EntryLinksSpec, PartitionLinksSpec, SortBy, SortDirection, TableBlueprint,
};
use crate::table_utils::{ColumnConfig, TableConfig, apply_table_style_fixes, cell_ui, header_ui};
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
    column_from_index: IntMap<egui::Id, usize>,
}

impl<'a> Columns<'a> {
    fn from(sorbet_schema: &'a SorbetSchema, column_blueprint_fn: &ColumnBlueprintFn<'_>) -> Self {
        let (columns, column_from_index) = sorbet_schema
            .columns
            .iter()
            .enumerate()
            .map(|(index, desc)| {
                let id = egui::Id::new(desc);
                let desc = desc.into();
                let blueprint = column_blueprint_fn(&desc);

                let column = Column {
                    id,
                    desc,
                    blueprint,
                };

                (column, (id, index))
            })
            .unzip();

        Self {
            columns,
            column_from_index,
        }
    }
}

impl Columns<'_> {
    fn iter(&self) -> impl Iterator<Item = &Column<'_>> + use<'_> {
        self.columns.iter()
    }

    fn index_from_id(&self, id: Option<egui::Id>) -> Option<usize> {
        let id = id?;
        self.column_from_index.get(&id).copied()
    }

    fn index_and_column_from_id(&self, id: Option<egui::Id>) -> Option<(usize, &Column<'_>)> {
        let index = id.and_then(|id| self.column_from_index.get(&id).copied())?;
        self.columns.get(index).map(|column| (index, column))
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
        egui_ctx: &egui::Context,
        session_ctx: &SessionContext,
        table_ref: impl Into<TableReference>,
    ) {
        let id = id_from_session_context_and_table(session_ctx, &table_ref.into());

        DataFusionAdapter::clear_state(egui_ctx, id);
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

    pub fn generate_partition_links(
        mut self,
        column_name: impl Into<String>,
        partition_id_column_name: impl Into<String>,
        origin: re_uri::Origin,
        dataset_id: EntryId,
    ) -> Self {
        self.initial_blueprint.partition_links = Some(PartitionLinksSpec {
            column_name: column_name.into(),
            partition_id_column_name: partition_id_column_name.into(),
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

        let requested_query_result = table_state.requested_query_result.lock();

        let mut is_table_update_in_progress = false;
        let query_result = match (
            requested_query_result.try_as_ref(),
            &table_state.last_query_results,
        ) {
            (Some(Ok(query_result)), _) => query_result,

            (Some(Err(err)), _) => {
                let error = format!("Could not load table: {err}");
                drop(requested_query_result);

                ui.horizontal(|ui| {
                    ui.error_label(&error);

                    if ui
                        .small_icon_button(&re_ui::icons::RESET, "Refresh")
                        .clicked()
                    {
                        // This will trigger a fresh query on the next frame.
                        Self::refresh(ui.ctx(), &session_ctx, table_ref);
                    }
                });
                return TableStatus::Error(error);
            }

            (None, Some(last_query_result)) => {
                // The new dataframe is still processing, but we have the previous one to display for now.
                is_table_update_in_progress = true;
                last_query_result
            }

            (None, None) => {
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
            ui,
            session_ctx.as_ref(),
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

        drop(requested_query_result);
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
        ui: &mut egui::Ui,
        session_ctx: &SessionContext,
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

        let mut sorted_columns = columns.iter().collect::<Vec<_>>();
        sorted_columns.sort_by_key(|c| c.blueprint.sort_key);
        let mut table_config = TableConfig::get_with_columns(
            ui.ctx(),
            static_id,
            sorted_columns.iter().map(|column| {
                ColumnConfig::new_with_visible(
                    column.id,
                    column.display_name(),
                    column.blueprint.default_visibility,
                )
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

        apply_table_style_fixes(ui.style_mut());

        let table_style = re_ui::TableStyle::Spacious;

        let mut row_height = viewer_ctx.tokens().table_row_height(table_style);

        // If the first column is a blob, we treat it as a thumbnail and increase the row height.
        // TODO(lucas): This is a band-aid fix and should be replaced with proper table blueprint
        let first_column = columns
            .index_from_id(table_config.visible_column_ids().next())
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
            ctx: viewer_ctx,
            table_style,
            query_result,
            migrated_fields: &migrated_fields,
            display_record_batches: &display_record_batches,
            columns: &columns,
            blueprint: table_blueprint,
            new_blueprint: &mut new_blueprint,
            table_config: &mut table_config,
            filter_state: &mut filter_state,
            row_height,
        };

        let visible_columns = table_delegate.table_config.visible_columns().count();
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
                Self::refresh(ui.ctx(), session_ctx, table_ref);
            }
            None => {}
        }

        // Calculate the maximum width of the row number column. Since we use monospace text,
        // calculating the width of the highest row number is sufficient.
        let max_row_number_width = (Self::row_number_text(num_rows)
            .into_galley(
                ui,
                Some(TextWrapMode::Extend),
                1000.0,
                FontSelection::Default,
            )
            .rect
            .width()
            + ui.tokens().table_cell_margin(table_style).sum().x)
            .ceil();

        egui_table::Table::new()
            .id_salt(session_id)
            .num_sticky_cols(1) // Row number column is sticky.
            .columns(
                iter::once(
                    egui_table::Column::new(max_row_number_width)
                        .resizable(false)
                        .range(Rangef::new(max_row_number_width, max_row_number_width))
                        .id(Id::new("row_number")),
                )
                .chain(
                    table_delegate
                        .table_config
                        .visible_column_ids()
                        .map(|id| egui_table::Column::new(200.0).resizable(true).id(id)),
                )
                .collect::<Vec<_>>(),
            )
            .headers(vec![egui_table::HeaderRow::new(
                ui.tokens().table_header_height(),
            )])
            .num_rows(num_rows)
            .show(ui, &mut table_delegate);

        table_config.store(ui.ctx());
        filter_state.store(ui.ctx(), session_id);

        new_blueprint
    }

    fn row_number_text(rows: u64) -> WidgetText {
        WidgetText::from(RichText::new(format_uint(rows)).weak().monospace())
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
    ctx: &'a ViewerContext<'a>,
    table_style: re_ui::TableStyle,
    query_result: &'a DataFusionQueryResult,
    migrated_fields: &'a Vec<Field>,
    display_record_batches: &'a Vec<DisplayRecordBatch>,
    columns: &'a Columns<'a>,
    blueprint: &'a TableBlueprint,
    new_blueprint: &'a mut TableBlueprint,
    table_config: &'a mut TableConfig,
    filter_state: &'a mut FilterState,
    row_height: f32,
}

impl egui_table::TableDelegate for DataFusionTableDelegate<'_> {
    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &HeaderCellInfo) {
        let tokens = ui.tokens();
        let table_style = self.table_style;

        if cell.group_index == 0 {
            header_ui(ui, table_style, false, |ui| ui.weak("#"));
        } else {
            ui.set_truncate_style();
            // Offset by one for the row number column.
            let column_index = cell.group_index - 1;

            let id = self.table_config.visible_column_ids().nth(column_index);

            if let Some((index, column)) = self.columns.index_and_column_from_id(id) {
                let column_field = &self.query_result.original_schema.fields[index];
                let column_physical_name = column_field.name();
                let column_display_name = column.display_name();

                let current_sort_direction = self.blueprint.sort_by.as_ref().and_then(|sort_by| {
                    (sort_by.column_physical_name.as_str() == column_physical_name)
                        .then_some(&sort_by.direction)
                });

                header_ui(ui, table_style, true, |ui| {
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
                                    current_sort_direction.map(SortDirection::icon)
                                {
                                    ui.add_space(-5.0);
                                    ui.small_icon(dir_icon, Some(tokens.table_sort_icon_color));
                                }

                                response
                            },
                            |ui| {
                                ui.set_height(ui.tokens().table_content_height(table_style));
                                egui::containers::menu::MenuButton::from_button(
                                    ui.small_icon_button_widget(
                                        &re_ui::icons::MORE,
                                        "More options",
                                    ),
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
                                                column_physical_name: column_physical_name
                                                    .to_owned(),
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
                                            ColumnFilter::default_for_column(Arc::clone(
                                                column_field,
                                            ))
                                    {
                                        if ui
                                            .icon_and_text_menu_item(
                                                &re_ui::icons::FILTER,
                                                "Filter",
                                            )
                                            .clicked()
                                        {
                                            self.filter_state.push_new_filter(column_filter);
                                        }
                                    }
                                });
                            },
                        )
                        .0
                })
                .inner
                .on_hover_ui(|ui| {
                    ui.with_optional_extras(|ui, show_extras| {
                        column_header_tooltip_ui(
                            ui,
                            &column.desc,
                            column_field,
                            &self.migrated_fields[index],
                            show_extras,
                        );
                    });
                });
            }
        }
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &CellInfo) {
        cell_ui(ui, self.table_style, false, |ui| {
            // find record batch
            let mut row_index = cell.row_nr as usize;

            ui.set_truncate_style();

            if cell.col_nr == 0 {
                // This is the row number column.
                ui.label(DataFusionTableWidget::row_number_text(cell.row_nr));
            } else {
                let col_index = cell.col_nr - 1; // Offset by one for the row number column.
                let id = self.table_config.visible_column_ids().nth(col_index);

                if let Some(col_idx) = self.columns.index_from_id(id) {
                    //TODO(ab): make an utility for that
                    for display_record_batch in self.display_record_batches {
                        let row_count = display_record_batch.num_rows();
                        if row_index < row_count {
                            // this is the one
                            let column = &display_record_batch.columns()[col_idx];

                            // TODO(#9029): it is _very_ unfortunate that we must provide a fake timeline, but
                            // avoiding doing so needs significant refactoring work.
                            column.data_ui(
                                self.ctx,
                                ui,
                                &re_viewer_context::external::re_chunk_store::LatestAtQuery::latest(
                                    TimelineName::new("unknown"),
                                ),
                                row_index,
                                None,
                            );

                            break;
                        } else {
                            row_index -= row_count;
                        }
                    }
                }
            }
        });
    }

    fn default_row_height(&self) -> f32 {
        self.row_height
    }
}
