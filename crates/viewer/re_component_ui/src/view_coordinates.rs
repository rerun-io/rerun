use re_sdk_types::components::ViewCoordinates;
use re_sdk_types::datatypes::ViewDir;
use re_types_core::reflection::Enum as _;
use re_ui::UiExt as _;
use re_ui::list_item::PropertyContent;
use re_viewer_context::{AppContext, MaybeMutRef};

use crate::{
    datatype_uis::{VariantAvailable, enum_variant_ui},
    response_utils::response_with_changes_of_inner,
};

/// Short, single-line representation: a drop-down over all named coordinate systems.
///
/// The selected text is the matching named coordinate system (e.g. `RDF`), or `custom` if the
/// axes don't span all three cardinal directions.
pub fn singleline_edit_or_view_view_coordinates(
    _ctx: &AppContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, ViewCoordinates>,
) -> egui::Response {
    let current = *value.as_ref();
    let selected_text = if current.handedness().is_ok() {
        current.describe_short()
    } else {
        "custom".to_owned()
    };

    let Some(mutable_value) = value.as_mut() else {
        return ui
            .add_enabled_ui(false, |ui| {
                egui::ComboBox::from_id_salt("viewcoordinates_preset")
                    .selected_text(selected_text)
                    .show_ui(ui, |_| {});
            })
            .response
            .on_hover_text(current.describe());
    };

    let mut selected = current;
    let response = response_with_changes_of_inner(
        egui::ComboBox::from_id_salt("viewcoordinates_preset")
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                let mut response: Option<egui::Response> = None;
                let mut add = |ui: &mut egui::Ui,
                               option: ViewCoordinates,
                               used_by: Option<&str>| {
                    let mut item = re_ui::ComboItem::new(format!(
                        "{} {} {}",
                        option[0].long(),
                        option[1].long(),
                        option[2].long()
                    ))
                    .selected(option == selected);
                    if let Some(used_by) = used_by {
                        // Right-aligned, subdued, like a help text.
                        item = item
                            .value(egui::RichText::new(used_by).color(ui.tokens().text_subdued));
                    }
                    let mut option_response = ui.add(item);
                    if option_response.clicked() {
                        selected = option;
                        option_response.mark_changed();
                    }
                    response = Some(match response.take() {
                        Some(response) => response | option_response,
                        None => option_response,
                    });
                };

                // Common conventions first, annotated with the software that uses them.
                for (option, used_by) in common_coordinate_systems() {
                    add(ui, option, Some(used_by));
                }

                ui.separator();

                // All remaining right-handed systems, without annotations.
                let common: Vec<_> = common_coordinate_systems().map(|(c, _)| c).collect();
                for option in all_right_handed_coordinate_systems() {
                    if !common.contains(&option) {
                        add(ui, option, None);
                    }
                }

                #[expect(clippy::unwrap_used)] // There is always at least one coordinate system.
                response.unwrap()
            }),
    );

    if selected != current {
        *mutable_value = selected;
    }

    response
}

/// The most common right-handed coordinate systems, each annotated with the software that uses it.
///
/// Left-handed systems are omitted entirely (Rerun doesn't support them yet), but any coordinate
/// system that is already set — common or not — is still detected and shown by name, see
/// [`singleline_edit_or_view_view_coordinates`].
fn common_coordinate_systems() -> impl Iterator<Item = (ViewCoordinates, &'static str)> {
    [
        (ViewCoordinates::RUB, "OpenGL, Blender, ARKit, Nerfstudio"),
        (ViewCoordinates::RDF, "OpenCV, Open3D, COLMAP"),
        (ViewCoordinates::FLU, "ROS"),
        (ViewCoordinates::FRD, "NED (aerospace, drones)"),
        (ViewCoordinates::RFU, "ENU (geospatial, world frame)"),
        (ViewCoordinates::LUF, "PyTorch3D"),
    ]
    .into_iter()
}

/// All right-handed named coordinate systems.
///
/// Left-handed systems are intentionally omitted (Rerun doesn't support them yet), but are still
/// detected and shown by name when already set, see [`singleline_edit_or_view_view_coordinates`].
fn all_right_handed_coordinate_systems() -> impl Iterator<Item = ViewCoordinates> {
    use ViewDir::{Back, Down, Forward, Left, Right, Up};
    use re_sdk_types::view_coordinates::Handedness;

    // One direction from each of the three cardinal axes, in all 6 axis assignments.
    const PERMUTATIONS: [[usize; 3]; 6] = [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ];

    [Up, Down]
        .into_iter()
        .flat_map(|i| {
            [Left, Right].into_iter().flat_map(move |j| {
                [Forward, Back].into_iter().flat_map(move |k| {
                    let dims = [i, j, k];
                    PERMUTATIONS
                        .into_iter()
                        .map(move |[a, b, c]| ViewCoordinates::new(dims[a], dims[b], dims[c]))
                })
            })
        })
        .filter(|coords| coords.handedness() == Ok(Handedness::Right))
}

/// Multi-line representation: a separate drop-down for each of the x/y/z axes.
pub fn multiline_edit_or_view_view_coordinates(
    _ctx: &AppContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, ViewCoordinates>,
) -> egui::Response {
    let editable = value.as_mut().is_some();
    let mut directions = (value.as_ref().0).0;
    let mut any_edit = false;

    let tokens = ui.tokens();
    let mut response: Option<egui::Response> = None;
    for (axis, name, color) in [
        (0usize, "x", tokens.axis_color_x),
        (1, "y", tokens.axis_color_y),
        (2, "z", tokens.axis_color_z),
    ] {
        let item_response = ui.list_item().interactive(false).show_flat(
            ui,
            PropertyContent::new(egui::RichText::new(name).color(color)).value_fn(|ui, _| {
                let axis_response = ui
                    .add_enabled_ui(editable, |ui| edit_single_axis(ui, axis, &mut directions))
                    .inner;
                any_edit |= axis_response.changed();
            }),
        );
        response = Some(match response {
            Some(response) => response | item_response,
            None => item_response,
        });
    }

    if let Some(mutable_value) = value.as_mut() {
        (mutable_value.0).0 = directions;
    }

    #[expect(clippy::unwrap_used)] // We always iterate over at least one axis.
    let mut response = response.unwrap();
    if any_edit {
        response.mark_changed();
    }
    response
}

fn edit_single_axis(
    ui: &mut egui::Ui,
    axis: usize,
    directions: &mut [ViewDir; 3],
) -> egui::Response {
    let previous_value = directions[axis];
    let mut selected_value = previous_value;
    let mut response = response_with_changes_of_inner(
        egui::ComboBox::from_id_salt(("viewcoordinates", axis))
            .selected_text(selected_value.to_string())
            .show_ui(ui, |ui| {
                ui.set_min_width(90.0);

                let variants = ViewDir::variants();

                let mut response =
                    enum_variant_ui(ui, &mut selected_value, variants[0], VariantAvailable::Yes);
                for variant in variants.iter().copied().skip(1) {
                    response |=
                        enum_variant_ui(ui, &mut selected_value, variant, VariantAvailable::Yes);
                }
                response
            }),
    )
    .on_hover_ui(|ui| {
        ui.markdown_ui(previous_value.docstring_md());
    });

    if selected_value != previous_value {
        apply_axis_edit(directions, axis, selected_value);
        response.mark_changed();
    }

    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::ComboBox,
            ui.is_enabled(),
            selected_value.to_string(),
        )
    });

    response
}

fn apply_axis_edit(directions: &mut [ViewDir; 3], edited_axis_idx: usize, selected_value: ViewDir) {
    directions[edited_axis_idx] = selected_value;

    for axis_idx in 0..directions.len() {
        if axis_idx != edited_axis_idx && directions[axis_idx] == selected_value {
            directions[axis_idx] = next_unused_view_dir(directions[axis_idx], directions, axis_idx);
        }
    }
}

fn next_unused_view_dir(
    current_value: ViewDir,
    directions: &[ViewDir; 3],
    axis_idx_to_replace: usize,
) -> ViewDir {
    let variants = ViewDir::variants();
    let start_index = variants
        .iter()
        .position(|variant| *variant == current_value)
        .unwrap_or(0);

    for variant in variants
        .iter()
        .cycle()
        .skip(start_index + 1)
        .take(variants.len())
        .copied()
    {
        if directions
            .iter()
            .enumerate()
            .all(|(axis_idx, direction)| axis_idx == axis_idx_to_replace || *direction != variant)
        {
            return variant;
        }
    }

    current_value
}
