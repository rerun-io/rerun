use egui::class::{Classes, HasClasses as _};
use egui::{Button, IntoAtoms, NumExt as _, Rect, Response, Sense, Vec2, WidgetAtomResponse};

use crate::theme::class;
use crate::theme::class::{ClassesExt as _, UiStackExt as _};
use crate::{ButtonVisuals, DesignTokens, icons};

/// The color scheme of a button.
///
/// Applied to an [`egui::Button`] via [`ButtonExt::variant`], which stores it as a class name that
/// [`crate::ReTheme`] resolves back into colors from the design tokens.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    Primary,
    Secondary,
    #[default]
    Ghost,
    Outlined,

    /// Filled with the brand blue, for the one action we want the user to take.
    Blue,

    /// Indicate that the thing this button represents is opened.
    Opened,

    /// The second selected look, for a button with more than one selected state to tell apart.
    ///
    /// Only reachable through [`ButtonExt::selected_variant`], since a selected button takes the
    /// ordinary selected look unless it says otherwise.
    SelectedAlt,
}

impl Variant {
    pub const CLASS_PRIMARY: &'static str = "rerun::button::primary";
    pub const CLASS_SECONDARY: &'static str = "rerun::button::secondary";
    pub const CLASS_GHOST: &'static str = "rerun::button::ghost";
    pub const CLASS_OUTLINED: &'static str = "rerun::button::outlined";
    pub const CLASS_BLUE: &'static str = "rerun::button::blue";
    pub const CLASS_OPENED: &'static str = "rerun::button::opened";

    /// The alternate selected fill (green), used by three-state toggle buttons.
    pub const CLASS_SELECTED_ALT: &'static str = "rerun::button::selected_alt";

    pub const fn class_name(self) -> &'static str {
        match self {
            Self::Primary => Self::CLASS_PRIMARY,
            Self::Secondary => Self::CLASS_SECONDARY,
            Self::Ghost => Self::CLASS_GHOST,
            Self::Outlined => Self::CLASS_OUTLINED,
            Self::Blue => Self::CLASS_BLUE,
            Self::Opened => Self::CLASS_OPENED,
            Self::SelectedAlt => Self::CLASS_SELECTED_ALT,
        }
    }

    fn from_class_name(name: &str) -> Option<Self> {
        match name {
            Self::CLASS_PRIMARY => Some(Self::Primary),
            Self::CLASS_SECONDARY => Some(Self::Secondary),
            Self::CLASS_GHOST => Some(Self::Ghost),
            Self::CLASS_OUTLINED => Some(Self::Outlined),
            Self::CLASS_BLUE => Some(Self::Blue),
            Self::CLASS_OPENED => Some(Self::Opened),
            Self::CLASS_SELECTED_ALT => Some(Self::SelectedAlt),
            _ => None,
        }
    }

    /// The variant a button asks for, if any.
    ///
    /// The last variant class wins, so chaining `.ghost().secondary()` yields [`Self::Secondary`].
    /// Being selected is not a variant of its own:
    /// egui sets [`egui::Button::CLASS_SELECTED`] on top of whatever the caller asked for, and the
    /// theme reads it separately.
    pub fn from_classes(classes: &Classes) -> Option<Self> {
        classes.last_class(Self::from_class_name)
    }

    pub fn visuals(self, tokens: &DesignTokens) -> ButtonVisuals {
        match self {
            Self::Primary => tokens.button_primary,
            Self::Secondary => tokens.button_secondary,
            Self::Ghost => tokens.button_ghost,
            Self::Outlined => tokens.button_outlined,
            Self::Blue => tokens.button_blue,
            Self::Opened => tokens.button_opened,
            Self::SelectedAlt => ButtonVisuals {
                // The loop color is one flat opaque token, with no lighter and darker step of its
                // own, so this fill cannot follow the pointer the way a variant's does.
                fill: tokens.loop_everything_color,
                fill_hovered: tokens.loop_everything_color,
                fill_pressed: tokens.loop_everything_color,
                ..tokens.selection
            },
        }
    }
}

/// The height (and thus padding) of a button.
///
/// Applied to an [`egui::Button`] via [`ButtonExt::size`], which stores it as a class name that
/// [`crate::ReTheme`] resolves into the button's padding and minimum size.
#[derive(Debug, Clone, Default, Copy, PartialEq)]
pub enum Size {
    Normal,
    Small,
    #[default]
    Tiny,
    Custom {
        height: f32,
    },
}

impl Size {
    pub const CLASS_NORMAL: &'static str = "rerun::size::normal";
    pub const CLASS_SMALL: &'static str = "rerun::size::small";
    pub const CLASS_TINY: &'static str = "rerun::size::tiny";

    /// Present on a button sized with [`Self::Custom`]. Such a button carries its exact height as
    /// its own `min_size`, so the theme must not impose the borrowed size's minimum on top.
    pub const CLASS_CUSTOM: &'static str = "rerun::size::custom";

    pub const fn custom(height: f32) -> Self {
        Self::Custom { height }
    }

    pub const fn height(self) -> f32 {
        match self {
            Self::Normal => 34.0,
            Self::Small => 26.0,
            Self::Tiny => 20.0,
            Self::Custom { height } => height,
        }
    }

    /// What is the size of the button when it's shown as an icon button?
    pub const fn icon_button_size(self) -> Vec2 {
        Vec2::splat(self.height())
    }

    pub const fn padding(self) -> Vec2 {
        let padding_normal = 8.0;
        let padding_small = 4.0;
        let x = match self {
            Self::Normal | Self::Small => padding_normal,
            Self::Tiny => padding_small,
            Self::Custom { height } => {
                if height >= Self::Small.height() {
                    padding_normal
                } else {
                    padding_small
                }
            }
        };
        Vec2::new(x, 0.0)
    }

    /// [`Self::Custom`] has no class of its own: its height travels as the button's `min_size`,
    /// and only its padding needs to reach the theme, so it borrows the class of the standard
    /// size with the same padding.
    pub const fn class_name(self) -> &'static str {
        match self {
            Self::Normal => Self::CLASS_NORMAL,
            Self::Small => Self::CLASS_SMALL,
            Self::Tiny => Self::CLASS_TINY,
            Self::Custom { height } => {
                if height >= Self::Small.height() {
                    Self::CLASS_SMALL
                } else {
                    Self::CLASS_TINY
                }
            }
        }
    }

    fn from_class_name(name: &str) -> Option<Self> {
        match name {
            Self::CLASS_NORMAL => Some(Self::Normal),
            Self::CLASS_SMALL => Some(Self::Small),
            Self::CLASS_TINY => Some(Self::Tiny),
            // Only allowed in `theme::class::UNCONTROLLED_EXTERNAL_UI`.
            _ if name == Button::CLASS_SMALL.as_str() => Some(Self::Small),
            _ => None,
        }
    }

    /// The size a button asks for, if any. The last size class wins.
    pub fn from_classes(classes: &Classes) -> Option<Self> {
        classes.last_class(Self::from_class_name)
    }

    /// The size a button in this [`egui::Ui`] gets, unless it asks for another one.
    ///
    /// Use this to reserve room for a button before showing it — the theme sizes it from the same
    /// classes.
    pub fn inherited(ui: &egui::Ui) -> Self {
        ui.stack().inherited(Self::from_classes).unwrap_or_default()
    }
}

/// Rerun's button styling, as extensions to [`egui::Button`].
///
/// Each method attaches a class name that [`crate::ReTheme`] turns into colors and padding.
/// A button that calls none of these keeps egui's default styling.
///
/// The size helpers are named `size_*` rather than `small`/`tiny`, because [`egui::Button`] has an
/// inherent `small()` that would silently shadow a trait method of the same name.
pub trait ButtonExt: Sized {
    /// A square button showing nothing but an icon.
    ///
    /// `alt_text` is the button's accessible name: what a screen reader announces, what
    /// `Harness::get_by_label` finds, and the only thing the MCP UI tools can search an icon
    /// button by. A button without one is unreachable by name, so it is required here.
    fn icon(icon: icons::Icon, alt_text: impl Into<String>) -> Button<'static> {
        Button::new(icon.as_image().alt_text(alt_text)).icon_button()
    }

    /// A button with a dropdown arrow after its contents.
    fn dropdown(atoms: impl IntoAtoms<'static>) -> Button<'static> {
        Button::new((atoms, icons::DROPDOWN_ARROW)).image_tint_follows_text_color(true)
    }

    fn variant(self, variant: Variant) -> Self;

    fn size(self, size: Size) -> Self;

    /// Make the button square, with no padding around its contents.
    fn icon_button(self) -> Self;

    /// Render the button with its hovered fill even when it isn't hovered.
    ///
    /// Useful to show that the button is engaged, e.g. while its menu popup is open.
    fn highlighted(self, highlighted: bool) -> Self;

    /// Show the button selected, styled as `variant` rather than with the selection accent.
    ///
    /// The button is selected as far as a screen reader is concerned — this only decides its
    /// colors. For a button with more than one selected look to tell apart, such as the loop
    /// button. Implies [`egui::Button::selected`], so it replaces that call rather than joining it.
    fn selected_variant(self, selected: bool, variant: Variant) -> Self;

    fn primary(self) -> Self {
        self.variant(Variant::Primary)
    }

    fn secondary(self) -> Self {
        self.variant(Variant::Secondary)
    }

    fn ghost(self) -> Self {
        self.variant(Variant::Ghost)
    }

    fn outlined(self) -> Self {
        self.variant(Variant::Outlined)
    }

    fn blue(self) -> Self {
        self.variant(Variant::Blue)
    }

    fn opened(self) -> Self {
        self.variant(Variant::Opened)
    }

    fn size_normal(self) -> Self {
        self.size(Size::Normal)
    }

    fn size_small(self) -> Self {
        self.size(Size::Small)
    }

    fn size_tiny(self) -> Self {
        self.size(Size::Tiny)
    }
}

impl ButtonExt for Button<'_> {
    fn variant(self, variant: Variant) -> Self {
        self.with_class(variant.class_name())
            .image_tint_follows_text_color(true)
    }

    fn size(self, size: Size) -> Self {
        let button = self
            .with_class(size.class_name())
            .image_tint_follows_text_color(true);

        // The class already carries the size to the theme, which is what sizes the button.
        // `Custom` is the exception: it borrows the class of the nearest standard size, so its
        // exact height has to travel on the widget.
        match size {
            Size::Custom { .. } => button
                .with_class(Size::CLASS_CUSTOM)
                .min_size(size.icon_button_size()),
            _ => button,
        }
    }

    fn icon_button(self) -> Self {
        self.with_class(class::ICON)
            .image_tint_follows_text_color(true)
    }

    fn highlighted(self, highlighted: bool) -> Self {
        self.with_class_if(class::HIGHLIGHTED, highlighted)
    }

    fn selected_variant(self, selected: bool, variant: Variant) -> Self {
        self.selected(selected)
            // The variant class goes on last, so it wins over any variant the button already asked
            // for — the point of the call is to restyle the selected state.
            .with_class_if(variant.class_name(), selected)
            .with_class_if(class::SELECTED_VARIANT, selected)
    }
}

/// Rerun's styling, as extensions to [`egui::DragValue`].
///
/// A drag value is a [`Button`] while dragged and a [`egui::TextEdit`] while typed into, and it
/// forwards its classes to both, so it is styled just like they are.
pub trait DragValueExt: Sized {
    fn variant(self, variant: Variant) -> Self;

    fn size(self, size: Size) -> Self;
}

impl DragValueExt for egui::DragValue<'_> {
    fn variant(self, variant: Variant) -> Self {
        self.with_class(variant.class_name())
    }

    fn size(self, size: Size) -> Self {
        self.with_class(size.class_name())
            .with_class_if(Size::CLASS_CUSTOM, matches!(size, Size::Custom { .. }))
            .min_size(size.icon_button_size())
    }
}

/// Show a [`Button`] that reveals more icon buttons (or other content) on hover.
///
/// `button` will be called multiple times on frames where the button is hovered.
///
/// Pass in the width the hover ui will need (usually this follows the formula
/// `count * button_width + (count - 1) * icon_spacing`)
pub fn with_hover_icon_buttons<'a, R>(
    ui: &mut egui::Ui,
    mut button: impl FnMut() -> Button<'a>,
    mut hover_buttons_width: f32,
    hover_buttons: impl FnOnce(&mut egui::Ui) -> R,
) -> (WidgetAtomResponse, Option<R>) {
    // Left and right spacing around the icons + some tolerance
    hover_buttons_width += ui.spacing().icon_spacing * 2.0 + 1.0;

    let clip_rect = ui.clip_rect();

    let calc_rect_with_buttons = |mut button_rect: Rect, available_rect: Rect| {
        let limit = available_rect
            .max
            .x
            .at_least(button_rect.max.x)
            .at_most(clip_rect.max.x);
        button_rect.max.x = (button_rect.max.x + hover_buttons_width).at_most(limit);
        button_rect
    };

    let id = ui.next_auto_id();
    let hovered = ui.read_response(id).is_some_and(|last| {
        let rect_with_buttons =
            calc_rect_with_buttons(last.interact_rect, ui.available_rect_before_wrap());
        ui.rect_contains_pointer(rect_with_buttons)
    });

    let mut atom_response = None;
    ui.add_visible(!hovered, |ui: &mut egui::Ui| {
        let atom_layout_response = button().atom_ui(ui);
        atom_response = Some(atom_layout_response.clone());
        atom_layout_response.response
    });
    let response = atom_response.expect("Should be set now");

    // Due to the interact_radius there would be a couple px where the cursor would trigger the
    // hover without actually being rect_contains_pointer, which looks confusing (hovered but
    // the icon button isn't shown). To mask, interact on top of the button so it doesn't get
    // the hover style:
    ui.interact(response.rect, id.with("hover_mask"), Sense::click());

    if !hovered {
        ui.skip_ahead_auto_ids(1);
        return (response, None);
    }

    let rect_with_buttons = calc_rect_with_buttons(response.rect, ui.available_rect_before_wrap());

    let mut extra_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect_with_buttons)
            .layout(egui::Layout::right_to_left(egui::Align::Min)),
    );
    extra_ui.spacing_mut().item_spacing.x = ui.spacing().icon_spacing;

    let button = button().truncate();

    let (response, icon_response) = egui::Sides::new()
        .spacing(ui.spacing().icon_spacing)
        .shrink_left()
        .show(
            &mut extra_ui,
            |ui| button.atom_ui(ui),
            |ui| {
                ui.add_space(ui.spacing().icon_spacing);
                hover_buttons(ui)
            },
        );

    (response, Some(icon_response))
}

/// Show a [`Button`] that reveals an icon button on hover.
///
/// `button` will be called multiple times on frames where the button is hovered.
pub fn with_hover_icon_button<'a>(
    ui: &mut egui::Ui,
    icon: Button<'static>,
    button: impl FnMut() -> Button<'a>,
    size: Size,
) -> (WidgetAtomResponse, Option<Response>) {
    let icon = icon.size(size);
    with_hover_icon_buttons(ui, button, size.icon_button_size().x, |ui| ui.add(icon))
}
