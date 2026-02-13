use re_ui::UiExt as _;
use re_viewer_context::{MaybeMutRef, ViewerContext};

use crate::response_utils::response_with_changes_of_inner;

/// Is a particular variant of an enum available for selection? If not, why?
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariantAvailable {
    /// The variant is available
    Yes,

    /// The variant is not available.
    No {
        /// The reason why the variant is not available, markdown formatted.
        reason_markdown: String,
    },
}

/// Trait for a type that can provide information about whether a particular variant of an enum is
/// available for selection.
pub trait VariantAvailableProvider<EnumT: re_types_core::reflection::Enum> {
    fn is_variant_enabled(ctx: &ViewerContext<'_>, variant: EnumT) -> VariantAvailable;
}

/// A variant available provider that makes all variants available.
struct AllEnumVariantAvailable<EnumT: re_types_core::reflection::Enum> {
    _phantom: std::marker::PhantomData<EnumT>,
}

impl<EnumT: re_types_core::reflection::Enum> VariantAvailableProvider<EnumT>
    for AllEnumVariantAvailable<EnumT>
{
    fn is_variant_enabled(_ctx: &ViewerContext<'_>, _variant: EnumT) -> VariantAvailable {
        VariantAvailable::Yes
    }
}

/// Edit or view an enum value. All variants are available.
pub fn edit_view_enum<EnumT: re_types_core::reflection::Enum + re_types_core::Component>(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    current_value: &mut MaybeMutRef<'_, EnumT>,
) -> egui::Response {
    edit_view_enum_with_variant_available::<EnumT, AllEnumVariantAvailable<EnumT>>(
        ctx,
        ui,
        current_value,
    )
}

/// Edit or view an enum value. The availability of each variant is determined by
/// the provided `VariantAvailableProvider` type.
pub fn edit_view_enum_with_variant_available<
    EnumT: re_types_core::reflection::Enum + re_types_core::Component,
    VariantAvailT: VariantAvailableProvider<EnumT>,
>(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    current_value: &mut MaybeMutRef<'_, EnumT>,
) -> egui::Response {
    let id_salt = EnumT::name().full_name();
    edit_view_enum_impl::<_, VariantAvailT>(ctx, ui, id_salt, current_value)
}

fn edit_view_enum_impl<
    EnumT: re_types_core::reflection::Enum,
    VariantAvailT: VariantAvailableProvider<EnumT>,
>(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    id_salt: &str,
    current_value: &mut MaybeMutRef<'_, EnumT>,
) -> egui::Response {
    if let Some(current_value) = current_value.as_mut() {
        let prev_selected_value = *current_value;

        let variants = EnumT::variants();

        if variants.is_empty() {
            ui.label("<no variants>")
        } else if variants.len() <= 2 {
            // Short version - only when there are 2 (or fewer) variants.
            // For more variants, this would become too wide, and we use a combobox instead.
            let mut response = ui
                .selectable_toggle(|ui| {
                    for variant in variants.iter().copied() {
                        variant_ui(
                            ui,
                            current_value,
                            variant,
                            VariantAvailT::is_variant_enabled(ctx, variant),
                        );
                    }
                })
                .response;
            if prev_selected_value != *current_value {
                response.mark_changed();
            }
            response
        } else {
            let mut combobox_response = egui::ComboBox::from_id_salt(id_salt)
                .selected_text(format!("{current_value}"))
                .height(250.0)
                .show_ui(ui, |ui| {
                    ui.set_min_width(60.0);

                    let mut iter = variants.iter().copied();
                    let Some(first) = iter.next() else {
                        return ui.label("<no variants>");
                    };

                    let mut response = crate::datatype_uis::enum_ui::variant_ui(
                        ui,
                        current_value,
                        first,
                        VariantAvailT::is_variant_enabled(ctx, first),
                    );
                    for variant in iter {
                        response |= variant_ui(
                            ui,
                            current_value,
                            variant,
                            VariantAvailT::is_variant_enabled(ctx, variant),
                        );
                    }
                    response
                });

            combobox_response.response = combobox_response.response.on_hover_ui(|ui| {
                ui.markdown_ui(prev_selected_value.docstring_md());
            });

            combobox_response.response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::ComboBox,
                    ui.is_enabled(),
                    current_value.to_string(),
                )
            });

            response_with_changes_of_inner(combobox_response)
        }
    } else {
        ui.add(egui::Label::new(current_value.to_string()).truncate())
    }
}

fn variant_ui<EnumT: re_types_core::reflection::Enum>(
    ui: &mut egui::Ui,
    current_value: &mut EnumT,
    variant: EnumT,
    variant_available: VariantAvailable,
) -> egui::Response {
    match variant_available {
        VariantAvailable::Yes => ui
            .selectable_value(current_value, variant, variant.to_string())
            .on_hover_ui(|ui| {
                ui.markdown_ui(variant.docstring_md());
            }),
        VariantAvailable::No {
            reason_markdown: reason,
        } => {
            ui.add_enabled_ui(false, |ui| {
                ui.selectable_value(current_value, variant, variant.to_string())
                    .on_disabled_hover_ui(|ui| {
                        ui.markdown_ui(&format!("{}\n\n{}", variant.docstring_md(), reason));
                    })
            })
            .inner
        }
    }
}
