//! UI code for `TimeSeriesView::visualizer_ui`, i.e. the visualizer list you get when selecting the an instance of the view.

use arrayvec::ArrayVec;
use re_component_ui::color_swatch::ColorSwatch;
use re_log_types::external::arrow::array::AsArray as _;
use re_sdk_types::archetypes::{SeriesLines, SeriesPoints};
use re_sdk_types::blueprint::archetypes::ActiveVisualizers;
use re_sdk_types::components::{self, Color, Name};
use re_sdk_types::{ComponentDescriptor, Loggable as _};
use re_ui::UiExt as _;
use re_ui::egui_ext::response_ext::ResponseExt as _;
use re_viewer_context::external::re_entity_db::InstancePath;
use re_viewer_context::{
    DataResultInteractionAddress, IdentifiedViewSystem as _, Item, SystemCommandSender as _,
};

use crate::point_visualizer_system::SeriesPointsSystem;

/// We only show this many colors directly.
const NUM_SHOWN_VISUALIZER_COLORS: usize = 2;

// Figma design for this can be found here:
// https://www.figma.com/design/eGATW7RubxdRrcEP9ITiVh/Any-scalars?node-id=956-7840&t=L1YFvJijAXXLlaBZ-0
// (accessible only by rerun team members)
pub fn visualizer_ui_element(
    ui: &mut egui::Ui,
    ctx: &re_viewer_context::ViewContext<'_>,
    node: &re_viewer_context::DataResultNode,
    pill_margin: egui::Margin,
    instruction: &re_viewer_context::VisualizerInstruction,
) {
    let entity_path = &node.data_result.entity_path;

    let (name_descr, color_descr, visibility_descr) =
        if instruction.visualizer_type == SeriesPointsSystem::identifier() {
            (
                SeriesPoints::descriptor_names(),
                SeriesPoints::descriptor_colors(),
                SeriesPoints::descriptor_visible_series(),
            )
        } else {
            // if instruction.visualizer_type == SeriesLinesSystem::identifier() {
            (
                SeriesLines::descriptor_names(),
                SeriesLines::descriptor_colors(),
                SeriesLines::descriptor_visible_series(),
            )
        };

    let query_result = re_view::latest_at_with_blueprint_resolved_data(
        ctx,
        None,
        &ctx.current_query(),
        &node.data_result,
        [
            name_descr.component,
            color_descr.component,
            visibility_descr.component,
        ],
        Some(instruction),
    );

    let display_name = extract_series_name(&query_result, &name_descr);
    let series_colors = extract_series_colors(ctx, &query_result, &color_descr);
    let visibility = extract_series_visibility(ctx, &query_result, &visibility_descr);
    let all_series_invisible = visibility.iter().all(|&visible| !visible);

    let mut frame = egui::Frame::default()
        .fill(ui.tokens().visualizer_list_pill_bg_color)
        .corner_radius(4.0)
        .inner_margin(pill_margin)
        .begin(ui);
    let frame_response = {
        // Time travel: retrieve the height of the previous frame so we can set it to the right side of egui::Sides.
        // In case of `shrink_left`, egui::Sides will render the right side first, and the content can't be centered
        // vertically since the height of the left side is unknown at that point. By setting the height explicitly,
        // we can vertically center the right side. It works since the height doesn't change.
        let frame_rect = frame.content_ui.response().rect;
        let frame_response = frame.content_ui.interact(
            frame_rect,
            ui.next_auto_id(),
            egui::Sense::hover() | egui::Sense::click(),
        );
        let previous_frame_height = frame_response.rect.height();

        // Show *either* visibility icon or color boxes.
        // Use `container_contains_pointer` instead of `container_hovered` because otherwise
        // we change behavior on mouse press-down which we don't want to do here.
        let show_visibility_icon =
            frame_response.container_contains_pointer() || all_series_invisible;

        egui::Sides::new().shrink_left().show(
            &mut frame.content_ui,
            |ui| {
                // Disable text selection so hovering the text only hovers the pill
                ui.style_mut().interaction.selectable_labels = false;

                let (title_color, path_color) = if all_series_invisible {
                    (
                        ui.tokens().visualizer_list_title_text_invisible_color,
                        ui.tokens().visualizer_list_path_text_invisible_color,
                    )
                } else {
                    (
                        ui.tokens().visualizer_list_title_text_color,
                        ui.tokens().visualizer_list_path_text_color,
                    )
                };

                // Visualizer name and entity path
                let full_path = entity_path.ui_string().trim_start_matches('/').to_owned();
                ui.vertical(|ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                    ui.label(egui::RichText::new(&display_name).color(title_color));
                    ui.label(egui::RichText::new(&full_path).size(10.5).color(path_color));
                });
            },
            |ui| {
                if previous_frame_height.is_sign_positive() {
                    ui.set_height(previous_frame_height);
                }

                if show_visibility_icon {
                    let response = if all_series_invisible {
                        ui.small_icon_button(&re_ui::icons::INVISIBLE, "Show series")
                    } else {
                        ui.small_icon_button(&re_ui::icons::VISIBLE, "Hide series")
                    };

                    if response.clicked() {
                        instruction.save_override(
                            ctx.viewer_ctx,
                            &visibility_descr,
                            &components::Visible::from(all_series_invisible),
                        );
                    }
                } else {
                    series_colors.ui(ui);
                }
            },
        );

        frame_response
    };

    let is_highlighted_via_item = ctx
        .viewer_ctx
        .hovered()
        .iter()
        .any(|(hovered, _hover_ctx)| {
            if let Item::DataResult(address) = hovered {
                address.view_id == ctx.view_id
                            && address.instance_path.entity_path == *entity_path // Don't care about instance id here.
                            && address.visualizer.is_none_or(|vid| vid == instruction.id)
            } else {
                false
            }
        });
    if is_highlighted_via_item || frame_response.container_contains_pointer() {
        frame.frame.fill = ui.tokens().visualizer_list_pill_bg_color_hovered;
    }

    frame.paint(ui);
    frame.allocate_space(ui);

    let item = Item::DataResult(DataResultInteractionAddress {
        view_id: ctx.view_id,
        instance_path: InstancePath::from(entity_path.clone()),
        visualizer: Some(instruction.id),
    });

    if frame_response.container_hovered() {
        ctx.viewer_ctx.selection_state().set_hovered(item.clone());
    }
    if frame_response.clicked() {
        ctx.viewer_ctx
            .command_sender()
            .send_system(re_viewer_context::SystemCommand::set_selection(item));
    }

    // Context menu with hide/show and remove actions.
    frame_response.context_menu(|ui| {
        context_menu_ui(
            ui,
            ctx,
            node,
            instruction,
            &visibility_descr,
            all_series_invisible,
        );
    });
}

fn context_menu_ui(
    ui: &mut egui::Ui,
    ctx: &re_viewer_context::ViewContext<'_>,
    node: &re_viewer_context::DataResultNode,
    instruction: &re_viewer_context::VisualizerInstruction,
    visibility_descr: &ComponentDescriptor,
    all_series_invisible: bool,
) {
    // Hide/show toggle
    let label = if all_series_invisible { "Show" } else { "Hide" };
    if ui.button(label).clicked() {
        instruction.save_override(
            ctx.viewer_ctx,
            visibility_descr,
            &components::Visible::from(all_series_invisible),
        );
        ui.close();
    }

    // Remove visualizer
    if ui.button("Remove").clicked() {
        let active_visualizers: Vec<_> = node
            .data_result
            .visualizer_instructions
            .iter()
            .filter(|v| v.id != instruction.id)
            .collect();

        let archetype = ActiveVisualizers::new(active_visualizers.iter().map(|v| v.id.0));
        let override_base_path = node.data_result.override_base_path().clone();
        ctx.save_blueprint_archetype(override_base_path, &archetype);
        ui.close();
    }
}

/// List of colors for a time series visualizer.
#[derive(Default)]
struct TimeSeriesColors {
    instance_count: usize,
    colors: ArrayVec<egui::Color32, NUM_SHOWN_VISUALIZER_COLORS>,
}

impl TimeSeriesColors {
    /// Draws color boxes (and an optional "+N" badge) right-aligned and vertically centered on
    /// the given `center_y`.
    fn ui(&self, ui: &mut egui::Ui) {
        if self.colors.is_empty() {
            return;
        }

        let spacing = 4.0;
        ui.spacing_mut().item_spacing.x = spacing;

        let num_boxes = if self.instance_count > 2 {
            1
        } else {
            self.colors.len()
        };

        // Draw "+N" badge when there are more than 2 instances
        if self.instance_count > 2 {
            let badge_text = format!("+{}", self.instance_count - 1);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            ui.label(
                egui::RichText::new(&badge_text)
                    .color(ui.tokens().visualizer_list_path_text_color)
                    .size(10.5),
            );
        }

        // Draw color boxes from right to left
        for color in self.colors[..num_boxes].iter().rev() {
            let rgba = re_sdk_types::datatypes::Rgba32::from(*color);
            let mut color_ref = re_viewer_context::MaybeMutRef::Ref(&rgba);
            ui.add(ColorSwatch::new(&mut color_ref));
        }
    }
}

/// Extracts the display name from already-queried results.
fn extract_series_name(
    query_result: &re_view::BlueprintResolvedLatestAtResults<'_>,
    name_descr: &ComponentDescriptor,
) -> String {
    let first_name = query_result.get_mono_with_fallback::<Name>(name_descr.component);
    // We might have already "injected" the instance number into the name of the series.
    // So we re-normalize the series name again.
    strip_instance_number(&first_name)
}

/// Extracts colors from already-queried results.
fn extract_series_colors(
    ctx: &re_viewer_context::ViewContext<'_>,
    query_result: &re_view::BlueprintResolvedLatestAtResults<'_>,
    color_descr: &ComponentDescriptor,
) -> TimeSeriesColors {
    let raw_color_cell = if let Some(color_cells) = query_result.get_raw_cell(color_descr.component)
    {
        color_cells
    } else {
        ctx.viewer_ctx
            .component_fallback_registry()
            .fallback_for(color_descr, query_result.query_context())
    };

    let Ok(color_components) = Color::from_arrow(&raw_color_cell) else {
        re_log::error_once!("Failed to cast color array to Color");
        return TimeSeriesColors::default();
    };

    let colors = color_components
        .iter()
        .map(|&value| value.into())
        .take(NUM_SHOWN_VISUALIZER_COLORS)
        .collect();

    TimeSeriesColors {
        instance_count: color_components.len(),
        colors,
    }
}

/// Extracts visibility from already-queried results.
fn extract_series_visibility(
    ctx: &re_viewer_context::ViewContext<'_>,
    query_result: &re_view::BlueprintResolvedLatestAtResults<'_>,
    visibility_descr: &ComponentDescriptor,
) -> Vec<bool> {
    let raw_visibility = query_result
        .get_raw_cell(visibility_descr.component)
        .unwrap_or_else(|| {
            ctx.viewer_ctx
                .component_fallback_registry()
                .fallback_for(visibility_descr, query_result.query_context())
        });

    raw_visibility
        .as_boolean_opt()
        .map(|bool_array| bool_array.iter().flatten().collect())
        .unwrap_or_default()
}

fn strip_instance_number(str: &str) -> String {
    if let Some(stripped) = str.strip_suffix(']').and_then(|s| {
        let i = s.rfind('[')?;
        s[i + 1..]
            .bytes()
            .all(|b| b.is_ascii_digit())
            .then_some(&s[..i])
    }) {
        format!("{stripped}[]")
    } else {
        str.to_owned()
    }
}

#[test]
fn test_strip_instance_number() {
    // Empty string
    assert_eq!(strip_instance_number(""), "");

    // No brackets at all
    assert_eq!(strip_instance_number("foo"), "foo");
    assert_eq!(strip_instance_number("hello world"), "hello world");

    // Valid instance numbers should be normalized to []
    assert_eq!(strip_instance_number("foo[0]"), "foo[]");
    assert_eq!(strip_instance_number("foo[1]"), "foo[]");
    assert_eq!(strip_instance_number("foo[123]"), "foo[]");

    // Empty brackets (no digits) should remain unchanged
    assert_eq!(strip_instance_number("foo[]"), "foo[]");

    // Non-digit content in brackets should remain unchanged
    assert_eq!(strip_instance_number("foo[abc]"), "foo[abc]");
    assert_eq!(strip_instance_number("foo[1a]"), "foo[1a]");
    assert_eq!(strip_instance_number("foo[a1]"), "foo[a1]");
    assert_eq!(strip_instance_number("foo[ ]"), "foo[ ]");
    assert_eq!(strip_instance_number("foo[1 2]"), "foo[1 2]");

    // Half-open brackets (only `[`) should remain unchanged
    assert_eq!(strip_instance_number("foo["), "foo[");
    assert_eq!(strip_instance_number("["), "[");
    assert_eq!(strip_instance_number("foo[123"), "foo[123");

    // Half-closed brackets (only `]`) should remain unchanged
    assert_eq!(strip_instance_number("foo]"), "foo]");
    assert_eq!(strip_instance_number("]"), "]");
    assert_eq!(strip_instance_number("123]"), "123]");

    // Multiple bracket pairs - only the last valid instance number is stripped
    assert_eq!(strip_instance_number("foo[0][1]"), "foo[0][]");
    assert_eq!(strip_instance_number("foo[abc][123]"), "foo[abc][]");

    // Nested or malformed brackets
    assert_eq!(strip_instance_number("foo[[0]]"), "foo[[0]]");
    assert_eq!(strip_instance_number("foo[0]["), "foo[0][");
    assert_eq!(strip_instance_number("foo][0]"), "foo][]");

    // Edge cases with brackets in the middle
    assert_eq!(strip_instance_number("foo[0]bar"), "foo[0]bar");
    assert_eq!(strip_instance_number("foo[0]bar[1]"), "foo[0]bar[]");
}
