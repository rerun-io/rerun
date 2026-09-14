//! Where an asset was registered from, opened from its card.

use re_log_types::external::re_types_core::SegmentId;
use re_redap_client::AssetLayer;
use re_ui::UiExt as _;
use re_ui::modal::{ModalHandler, ModalWrapper};

/// Modal showing the source URIs an asset was registered from.
///
/// Read-only, since the server has no call that changes an asset's source URI. Laid out like
/// [`crate::register_asset_modal::RegisterAssetModal`], which is where those URIs were entered.
#[derive(Default)]
pub struct AssetSourceModal {
    modal: ModalHandler,

    state: Option<State>,
}

/// The asset the modal is showing while it is open.
struct State {
    asset_id: SegmentId,

    /// One per layer of the asset, sorted by layer name.
    layers: Vec<AssetLayer>,
}

impl AssetSourceModal {
    pub fn open(&mut self, asset_id: SegmentId, layers: Vec<AssetLayer>) {
        self.state = Some(State { asset_id, layers });
        self.modal.open();
    }

    pub fn ui(&mut self, ui: &egui::Ui) {
        let Some(state) = &self.state else {
            return;
        };

        self.modal.ui(
            ui.ctx(),
            || ModalWrapper::new("Source URI"),
            |ui| {
                ui.label(format!("Where {} was registered from.", state.asset_id));
                ui.add_space(8.0);

                if state.layers.is_empty() {
                    ui.warning_label("The server reported no source for this asset.");
                    return;
                }

                for (index, layer) in state.layers.iter().enumerate() {
                    if index > 0 {
                        ui.add_space(8.0);
                    }

                    // A single layer is the common case, and its name says nothing useful, so only
                    // show layer names when there is more than one.
                    ui.strong(if state.layers.len() > 1 {
                        layer.name.as_str()
                    } else {
                        "Source URI"
                    });

                    read_only_field_ui(ui, index, &layer.storage_url);
                }

                ui.add_space(8.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(re_ui::ReButton::new("Copy").small())
                        .on_hover_text("Copy every address on this asset")
                        .clicked()
                    {
                        let addresses = state
                            .layers
                            .iter()
                            .map(|layer| layer.storage_url.as_str())
                            .collect::<Vec<_>>()
                            .join("\n");

                        ui.copy_text(addresses);
                    }
                });
            },
        );
    }
}

// TODO(isse): Make this a capability of `ReTextEdit`?
/// A value the user can select and copy but not edit, styled like an editable field.
///
/// egui paints a read-only `TextEdit` with a transparent background, so the frame is drawn here
/// instead, and the text is paler than in an editable field. The field starts scrolled to the end,
/// since that is where one source URI differs from another.
fn read_only_field_ui(ui: &mut egui::Ui, id_salt: usize, value: &str) {
    let tokens = ui.tokens();
    let field = ui.visuals().widgets.inactive;

    egui::Frame::new()
        .fill(ui.visuals().text_edit_bg_color.unwrap_or(tokens.card_fill))
        .stroke(field.bg_stroke)
        .corner_radius(field.corner_radius)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.take_available_width();

            egui::ScrollArea::horizontal()
                .id_salt(id_salt)
                .stick_to_right(true)
                .show(ui, |ui| {
                    let mut value = value;

                    ui.add(
                        egui::TextEdit::singleline(&mut value)
                            .frame(egui::Frame::NONE)
                            // Lay out the whole URI, and let the scroll area move within it.
                            .clip_text(false)
                            .text_color(tokens.text_readonly),
                    );
                });
        });
}

#[cfg(test)]
mod tests {
    use re_log_types::external::re_types_core::LayerName;

    use super::*;

    /// An asset registered from a single layer shows its source URI without a layer name.
    #[test]
    fn a_single_layer_shows_one_source_uri() {
        run_test(
            vec![layer("base", "s3://rerun-datasets/droid/shared_mesh.rrd")],
            "asset_source_modal__single_layer",
        );
    }

    /// An asset registered from several layers shows one field per layer, each with its name.
    #[test]
    fn several_layers_show_a_named_field_each() {
        run_test(
            vec![
                layer("base", "s3://rerun-datasets/droid/shared_mesh.rrd"),
                layer("high", "s3://rerun-datasets/droid/shared_mesh_high.rrd"),
            ],
            "asset_source_modal__multiple_layers",
        );
    }

    /// An asset the server reported no layers for shows a warning instead of any field.
    #[test]
    fn an_asset_with_no_layers_shows_a_warning() {
        run_test(vec![], "asset_source_modal__no_layers");
    }

    fn layer(name: &str, storage_url: &str) -> AssetLayer {
        AssetLayer {
            name: LayerName::try_new(name).expect("valid layer name"),
            storage_url: storage_url.to_owned(),
        }
    }

    fn run_test(layers: Vec<AssetLayer>, snapshot_name: &str) {
        let mut modal = AssetSourceModal::default();
        modal.open(SegmentId::from("shared_mesh"), layers);

        let mut harness = re_ui::testing::new_harness(
            re_ui::testing::TestOptions::Gui,
            egui::Vec2::new(500.0, 300.0),
        )
        .build_ui(|ui| {
            re_ui::apply_style_and_install_loaders(ui.ctx());

            modal.ui(ui);
        });

        harness.run();
        harness.snapshot(snapshot_name);
    }
}
