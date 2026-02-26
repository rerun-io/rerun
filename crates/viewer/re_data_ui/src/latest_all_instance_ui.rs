use re_format::format_plural_s;
use re_log_types::{EntityPath, Instance};
use re_query::LatestAllComponentResults;
use re_sdk_types::ComponentIdentifier;
use re_ui::UiExt as _;
use re_viewer_context::{UiLayout, ViewerContext};

use super::{DataUi, LatestAtInstanceResult};

/// All the values of a specific [`re_log_types::ComponentPath`].
///
/// This can include multiple values logged over each other onto the same time point
/// (common e.g. for TF-style transforms).
///
/// This can also include multiple _instances_ per hit (e.g. multiple points in a point cloud).
#[derive(Clone)]
pub struct LatestAllInstanceResult<'a> {
    /// `camera / "left" / points / #42`
    pub entity_path: EntityPath,

    /// e.g. `Points3D:color`
    pub component: ComponentIdentifier,

    /// A specific instance (e.g. point in a point cloud), or [`Instance::ALL`] of them.
    pub instance: Instance,

    pub hits: &'a LatestAllComponentResults,
}

impl DataUi for LatestAllInstanceResult<'_> {
    fn data_ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        ui_layout: UiLayout,
        query: &re_chunk_store::LatestAtQuery,
        db: &re_entity_db::EntityDb,
    ) {
        let Self {
            entity_path,
            component,
            instance,
            hits,
        } = self.clone();

        re_tracing::profile_function!(component);

        ui.sanity_check();

        let engine = db.storage_engine();

        let time = hits.time();
        let timeline = query.timeline();
        let time_type = db.timeline_type(&query.timeline());
        let formatted_time = time_type.format(time, ctx.app_options().timestamp_format);

        if !ui_layout.is_single_line() {
            // Display time and other diagnostic information as a preamble:
            if time.is_static() {
                // Note: latest-all only ever return ONE static event
                re_log::debug_assert_eq!(hits.num_rows(), 1);

                let static_message_count = engine
                    .store()
                    .num_physical_static_events_for_component(&entity_path, component);

                if static_message_count > 1 {
                    ui.warning_label(format!(
                        "Logged {} as static",
                        format_plural_s(static_message_count, "time")
                    ))
                    .on_hover_text(
                        "When a static component is logged multiple times, only the last value \
                            is stored. Previously logged values are overwritten and not \
                            recoverable.",
                    );
                }

                let temporal_message_count = engine
                    .store()
                    .num_physical_temporal_events_for_component_on_all_timelines(
                        &entity_path,
                        component,
                    );
                if temporal_message_count > 0 {
                    ui.error_label(format!(
                        "Static component also has {}",
                        re_format::format_plural_s(temporal_message_count, "temporal event")
                    ))
                    .on_hover_text(
                        "Components should be logged either as static or on timelines, but \
                        never both. Values for static components logged to timelines cannot be \
                        displayed.",
                    );
                }
            } else {
                // Temporal component
                if 1 < hits.num_rows() {
                    ui.horizontal(|ui| {
                        ui.add(re_ui::icons::COMPONENT_TEMPORAL.as_image());
                        ui.label(format!(
                            "Logged {} at {timeline}={formatted_time}",
                            format_plural_s(hits.num_rows(), "time")
                        ));
                    });
                }
            }
        }

        if let Some(unit) = hits.try_as_unit() {
            // Common case: latest-all == latest-as

            LatestAtInstanceResult {
                entity_path,
                component,
                instance,
                unit: &unit,
            }
            .data_ui(ctx, ui, ui_layout, query, db);
        } else {
            // Many hits on the same time point (e.g. transforms):

            if ui_layout.is_single_line() {
                if time.is_static() {
                    ui.label(format!(
                        "Logged {} as static",
                        format_plural_s(hits.num_rows(), "time")
                    ));
                } else {
                    ui.label(format!(
                        "Logged {} at {timeline}={formatted_time}",
                        format_plural_s(hits.num_rows(), "time")
                    ));
                }
            } else {
                // TODO(#8214): full nested timeline support
                for unit in hits.iter_units() {
                    // Showing the row-id here wouldn't be very useful.
                    ui.push_id(unit.row_id(), |ui| {
                        ui.add_space(8.0);
                        LatestAtInstanceResult {
                            entity_path: entity_path.clone(),
                            component,
                            instance,
                            unit: &unit,
                        }
                        .data_ui(ctx, ui, ui_layout, query, db);
                    });
                }
            }
        }

        ui.sanity_check();
    }
}
