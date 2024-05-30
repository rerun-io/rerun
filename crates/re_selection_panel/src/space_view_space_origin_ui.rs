use std::ops::ControlFlow;

use egui::{Key, NumExt as _, Ui};

use re_log_types::EntityPath;
use re_ui::{list_item, ReUi, SyntaxHighlighting};
use re_viewer_context::ViewerContext;
use re_viewport_blueprint::{default_created_space_views, SpaceViewBlueprint};

/// State of the space origin widget.
#[derive(Default, Clone)]
enum SpaceOriginEditState {
    #[default]
    NotEditing,

    Editing(EditState),
}

#[derive(Clone)]
struct EditState {
    /// The string currently entered by the user.
    origin_string: String,

    /// Did we just enter editing mode?
    entered_editing: bool,

    /// The index of the currently selected suggestion (for keyboard navigation).
    selected_suggestion: Option<usize>,
}

/// Display the space origin of a space view.
pub(crate) fn space_view_space_origin_widget_ui(
    ui: &mut Ui,
    ctx: &ViewerContext<'_>,
    space_view: &SpaceViewBlueprint,
) {
    let is_editing_id = ui.make_persistent_id(space_view.id.hash());
    let mut state: SpaceOriginEditState =
        ui.memory_mut(|mem| mem.data.get_temp(is_editing_id).unwrap_or_default());

    match &mut state {
        SpaceOriginEditState::NotEditing => {
            let mut space_origin_string = space_view.space_origin.to_string();
            let output = egui::TextEdit::singleline(&mut space_origin_string).show(ui);

            if output.response.gained_focus() {
                state = SpaceOriginEditState::Editing(EditState {
                    origin_string: space_origin_string,
                    entered_editing: true,
                    selected_suggestion: None,
                });
            }
        }
        SpaceOriginEditState::Editing(edit_state) => {
            let control_flow =
                space_view_space_origin_widget_editing_ui(ui, ctx, space_view, edit_state);

            match control_flow {
                ControlFlow::Break(Some(new_space_origin)) => {
                    space_view.set_origin(ctx, &new_space_origin);
                    state = SpaceOriginEditState::NotEditing;
                }
                ControlFlow::Break(None) => {
                    state = SpaceOriginEditState::NotEditing;
                }
                ControlFlow::Continue(()) => {
                    // Keep editing
                    edit_state.entered_editing = false;
                }
            }
        }
    }

    ui.memory_mut(|mem| mem.data.insert_temp(is_editing_id, state));
}

/// Display the space origin of a space view with it is in edit mode.
fn space_view_space_origin_widget_editing_ui(
    ui: &mut Ui,
    ctx: &ViewerContext<'_>,
    space_view: &SpaceViewBlueprint,
    state: &mut EditState,
) -> ControlFlow<Option<EntityPath>, ()> {
    let mut control_flow = ControlFlow::Continue(());

    let popup_id = ui.make_persistent_id("suggestions");

    //
    // Build and filter the suggestion lists
    //

    // All suggestions for this class of space views.
    // TODO(#4895): we should have/use a much simpler heuristic API to get a list of compatible entity sub-tree
    let space_view_suggestions = default_created_space_views(ctx)
        .into_iter()
        .filter(|this_space_view| {
            this_space_view.class_identifier() == space_view.class_identifier()
        })
        .collect::<Vec<_>>();

    // Filtered suggestions based on the current text edit content.
    let filtered_space_view_suggestions = space_view_suggestions
        .iter()
        .filter(|suggested_space_view| {
            suggested_space_view
                .space_origin
                .to_string()
                .contains(&state.origin_string)
        })
        .collect::<Vec<_>>();

    //
    // Move cursor with keyboard (must happen before text edit to capture the keystrokes
    //

    let mut arrow_down =
        ui.input_mut(|i| i.count_and_consume_key(Default::default(), Key::ArrowDown));
    let arrow_up = ui.input_mut(|i| i.count_and_consume_key(Default::default(), Key::ArrowUp));

    // force spawn a selected suggestion if the down arrow is pressed
    if arrow_down > 0 && state.selected_suggestion.is_none() {
        state.selected_suggestion = Some(0);
        arrow_down -= 1;
    }

    state.selected_suggestion = state.selected_suggestion.map(|mut selected_suggestion| {
        selected_suggestion = selected_suggestion
            .saturating_add(arrow_down)
            .saturating_sub(arrow_up);
        if !space_view_suggestions.is_empty() && !filtered_space_view_suggestions.is_empty() {
            selected_suggestion =
                selected_suggestion.at_most(filtered_space_view_suggestions.len() - 1);
        }
        selected_suggestion
    });

    //
    // Handle enter key when a suggestion is selected
    //

    let enter_key_hit = ui.input(|i| i.key_pressed(egui::Key::Enter));

    if let Some(selected_suggestion) = state.selected_suggestion {
        if enter_key_hit {
            if let Some(suggestion) = filtered_space_view_suggestions.get(selected_suggestion) {
                let origin = &suggestion.space_origin;
                state.origin_string = origin.to_string();
                control_flow = ControlFlow::Break(Some(origin.clone()));
            }
        }
    }

    //
    // Draw the text edit
    //

    let mut output = egui::TextEdit::singleline(&mut state.origin_string).show(ui);

    if state.entered_editing {
        output.response.request_focus();
        let min = egui::text::CCursor::new(0);
        let max = egui::text::CCursor::new(state.origin_string.len());
        let new_range = egui::text::CCursorRange::two(min, max);
        output.state.cursor.set_char_range(Some(new_range));
        output.state.store(ui.ctx(), output.response.id);
    }

    if output.response.changed() {
        space_view.set_origin(ctx, &state.origin_string.clone().into());
    }

    if output.response.lost_focus() && enter_key_hit && control_flow.is_continue() {
        control_flow = ControlFlow::Break(Some(state.origin_string.clone().into()));
    }

    //
    // Display popup with suggestions
    //

    if output.response.has_focus() {
        ui.memory_mut(|mem| mem.open_popup(popup_id));
    }

    let suggestions_ui = |ui: &mut egui::Ui| {
        for (idx, suggested_space_view) in filtered_space_view_suggestions.iter().enumerate() {
            let response = list_item::ListItem::new(ctx.re_ui)
                .force_hovered(state.selected_suggestion == Some(idx))
                .show_flat(
                    ui,
                    list_item::LabelContent::new(
                        suggested_space_view
                            .space_origin
                            .syntax_highlighted(ui.style()),
                    ),
                );

            if response.hovered() {
                state.selected_suggestion = None;
            }

            if response.clicked() {
                control_flow = ControlFlow::Break(Some(suggested_space_view.space_origin.clone()));
            }
        }

        let excluded_count = space_view_suggestions.len() - filtered_space_view_suggestions.len();
        if excluded_count > 0 {
            list_item::ListItem::new(ctx.re_ui)
                .interactive(false)
                .show_flat(
                    ui,
                    list_item::LabelContent::new(format!("{excluded_count} hidden suggestions"))
                        .weak(true)
                        .italics(true),
                );
        }
    };

    ReUi::list_item_popup(ui, popup_id, &output.response, 4.0, suggestions_ui);

    if control_flow.is_continue() && !ui.memory(|mem| mem.is_popup_open(popup_id)) {
        control_flow = ControlFlow::Break(None);
    };

    control_flow
}
