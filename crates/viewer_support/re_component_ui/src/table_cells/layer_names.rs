use egui::AtomExt as _;
use re_sdk_types::components::LayerName;
use re_ui::{ButtonExt as _, Size, UiExt as _, Variant, icons};
use re_viewer_context::{AppContext, MaybeMutRef};

/// Show each layer name as a tag.
///
/// In a wrapping layout, such as the fields of a card, all tags are shown and wrap onto new lines.
///
/// Otherwise the tags are on one line.
/// If they don't fit in the available width, the first tag that doesn't fit is truncated,
/// the ones after it are left out, and a `+N` label shows how many are left out.
/// Hovering the cell then lists all layer names.
/// While hovered, a button at the right edge copies all layer names.
pub fn layer_names(
    _ctx: &AppContext<'_>,
    ui: &mut egui::Ui,
    layer_names: &mut MaybeMutRef<'_, Vec<LayerName>>,
) -> egui::Response {
    let gap = 4.0;

    if ui.layout().main_wrap() {
        return wrapped_layer_names(ui, layer_names, gap);
    }

    let copy_button_size = Size::Tiny;
    let copy_button_id = egui::IdSalt::new("copy_button");
    let show_copy_button = ui.rect_contains_pointer(ui.max_rect());

    // The width left for tags. `WidgetAtom` adds `gap` before the `Atom::grow` too.
    let mut available_width = ui.max_rect().width() - gap;
    if show_copy_button {
        available_width -= copy_button_size.height() + gap;
    }

    let measure_width = |atom: egui::Atom<'_>| {
        atom.into_sized(
            ui,
            egui::Vec2::INFINITY,
            None,
            egui::FontSelection::default(),
        )
        .size
        .x
    };
    let measure_tag_width = |tag: &egui::ContainerAtom<'static>| {
        measure_width(egui::Atom::container(
            tag.clone().wrap_mode(egui::TextWrapMode::Extend),
        ))
    };
    let num_left_out_label = |num_left_out: usize| {
        egui::Atom::from(egui::RichText::new(format!("+{num_left_out}")).weak())
    };

    let min_truncated_tag_width = measure_tag_width(&layer_tag(ui, "…"));

    let tags: Vec<_> = layer_names
        .iter()
        .map(|layer_name| layer_tag(ui, layer_name.to_string()))
        .collect();

    // Count the tags that fit in full, and check if the next one fits truncated.
    let mut num_full = 0;
    let mut truncate_next = false;
    for tag in &tags {
        // Room for the `+N` label that follows this tag if the next one doesn't fit.
        let num_after = tags.len() - num_full - 1;
        let label_width = if num_after == 0 {
            0.0
        } else {
            measure_width(num_left_out_label(num_after)) + gap
        };

        let tag_width = measure_tag_width(tag);
        if available_width < tag_width + label_width {
            truncate_next = min_truncated_tag_width + label_width <= available_width;
            break;
        }
        num_full += 1;
        available_width -= tag_width + gap;
    }
    let num_shown = num_full + usize::from(truncate_next);
    let num_left_out = tags.len() - num_shown;
    let any_left_out = num_full < tags.len();

    let mut atoms = egui::Atoms::default();
    for (index, tag) in tags.into_iter().take(num_shown).enumerate() {
        if index < num_full {
            atoms.push_right(egui::Atom::container(
                tag.wrap_mode(egui::TextWrapMode::Extend),
            ));
        } else {
            atoms.push_right(
                egui::Atom::container(tag.wrap_mode(egui::TextWrapMode::Truncate))
                    .atom_shrink(true),
            );
        }
    }
    if num_left_out > 0 {
        atoms.push_right(num_left_out_label(num_left_out));
    }
    atoms.push_right(egui::Atom::grow());
    if show_copy_button {
        atoms.push_right(egui::Atom::custom(
            copy_button_id,
            egui::Vec2::splat(copy_button_size.height()),
        ));
    }

    let atom_response = egui::WidgetAtom::new(atoms)
        .sense(egui::Sense::hover())
        .gap(gap)
        .wrap_mode(egui::TextWrapMode::Extend)
        .min_size(ui.max_rect().size())
        .show(ui);

    let copy_button_hovered = if let Some(copy_button_rect) = atom_response.rect(copy_button_id) {
        let copy_button = ui
            .place(
                copy_button_rect,
                egui::Button::icon(icons::COPY, "Copy")
                    .size(copy_button_size)
                    .variant(Variant::Ghost),
            )
            .on_hover_text("Copy");
        if copy_button.clicked() {
            copy_layer_names(ui, layer_names);
        }
        copy_button.hovered()
    } else {
        false
    };

    let mut response = atom_response.response;
    if any_left_out && !copy_button_hovered {
        response = response.on_hover_ui(|ui| {
            for layer_name in layer_names.iter() {
                ui.label(layer_name.as_str());
            }
        });
    }

    response
}

/// Show each layer name as its own tag, so the layout wraps between tags.
///
/// The spacing after the last tag is the caller's spacing.
fn wrapped_layer_names(ui: &mut egui::Ui, layer_names: &[LayerName], gap: f32) -> egui::Response {
    let item_spacing = ui.spacing().item_spacing.x;

    let response = layer_names
        .iter()
        .enumerate()
        .map(|(index, layer_name)| {
            ui.spacing_mut().item_spacing.x = if index + 1 == layer_names.len() {
                item_spacing
            } else {
                gap
            };
            let tag = layer_tag(ui, layer_name.to_string()).wrap_mode(egui::TextWrapMode::Extend);
            egui::WidgetAtom::new(egui::Atom::container(tag))
                .sense(egui::Sense::hover())
                .show(ui)
                .response
        })
        .reduce(|a, b| a.union(b));
    ui.spacing_mut().item_spacing.x = item_spacing;

    response.unwrap_or_else(|| ui.weak("Empty"))
}

/// A layer name in a framed tag.
fn layer_tag(ui: &egui::Ui, text: impl egui::IntoAtoms<'static>) -> egui::ContainerAtom<'static> {
    let tokens = ui.tokens();
    let frame = egui::Frame::new()
        .fill(tokens.form_selectable_bg_color)
        .stroke(tokens.divider_stroke)
        .corner_radius(tokens.normal_corner_radius())
        .inner_margin(egui::Margin::symmetric(6, 1));
    egui::ContainerAtom::new(text).frame(frame)
}

fn copy_layer_names(ui: &egui::Ui, layer_names: &[LayerName]) {
    let copy_text = layer_names
        .iter()
        .map(|layer_name| layer_name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    ui.copy_text(copy_text.clone());
    re_log::info!("Copied {copy_text:?} to clipboard");
}
