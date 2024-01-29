use eframe::emath::NumExt;
use egui::{Key, Ui};

use re_ui::{ReUi, SyntaxHighlighting};
use re_viewer_context::ViewerContext;
use re_viewport::SpaceViewBlueprint;

/// State of the space origin widget.
#[derive(Default, Clone)]
enum SpaceOriginEditState {
    #[default]
    NotEditing,

    Editing {
        /// The string currently entered by the user.
        origin_string: String,

        /// Did we just enter editing mode?
        entered_editing: bool,

        /// The index of the currently selected suggestion (for keyboard navigation).
        selected_suggestion: Option<usize>,
    },
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
                state = SpaceOriginEditState::Editing {
                    origin_string: space_origin_string,
                    entered_editing: true,
                    selected_suggestion: None,
                };
            }
        }
        SpaceOriginEditState::Editing {
            origin_string,
            entered_editing,
            selected_suggestion,
        } => {
            let keep_editing = space_view_space_origin_widget_editing_ui(
                ui,
                ctx,
                origin_string,
                *entered_editing,
                space_view,
                selected_suggestion,
            );

            if keep_editing {
                *entered_editing = false;
            } else {
                state = SpaceOriginEditState::NotEditing;
            }
        }
    }

    ui.memory_mut(|mem| mem.data.insert_temp(is_editing_id, state));
}

/// Display the space origin of a space view with it is in edit mode.
fn space_view_space_origin_widget_editing_ui(
    ui: &mut Ui,
    ctx: &ViewerContext<'_>,
    space_origin_string: &mut String,
    entered_editing: bool,
    space_view: &SpaceViewBlueprint,
    selected_suggestion: &mut Option<usize>,
) -> bool {
    let mut keep_editing = true;

    //
    // Build and filter the suggestion lists
    //

    // All suggestions for this class of space views.
    // TODO(#4895): we should have/use a much simpler heuristic API to get a list of compatible entity sub-tree
    let space_view_suggestions =
        re_viewport::space_view_heuristics::default_created_space_views(ctx)
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
                .contains(&*space_origin_string)
        })
        .collect::<Vec<_>>();

    //
    // Move cursor with keyboard (must happen before text edit to capture the keystrokes
    //

    let mut arrow_down =
        ui.input_mut(|i| i.count_and_consume_key(Default::default(), Key::ArrowDown));
    let arrow_up = ui.input_mut(|i| i.count_and_consume_key(Default::default(), Key::ArrowUp));

    // force spawn a selected suggestion if the down arrow is pressed
    if arrow_down > 0 && selected_suggestion.is_none() {
        *selected_suggestion = Some(0);
        arrow_down -= 1;
    }

    *selected_suggestion = selected_suggestion.map(|mut selected_suggestion| {
        selected_suggestion = selected_suggestion
            .saturating_add(arrow_down)
            .saturating_sub(arrow_up);
        if !space_view_suggestions.is_empty() {
            selected_suggestion =
                selected_suggestion.at_most(filtered_space_view_suggestions.len() - 1);
        }
        selected_suggestion
    });

    //
    // Handle enter key when a suggestion is selected
    //

    let enter_key_hit = ui.input(|i| i.key_pressed(egui::Key::Enter));

    if let Some(selected_suggestion) = selected_suggestion {
        if enter_key_hit {
            *space_origin_string = filtered_space_view_suggestions[*selected_suggestion]
                .space_origin
                .to_string();
            keep_editing = false;
        }
    }

    //
    // Draw the text edit
    //

    let mut output = egui::TextEdit::singleline(space_origin_string).show(ui);

    if entered_editing {
        output.response.request_focus();
        let min = egui::text::CCursor::new(0);
        let max = egui::text::CCursor::new(space_origin_string.len());
        let new_range = egui::text::CCursorRange::two(min, max);
        output.state.cursor.set_char_range(Some(new_range));
        output.state.store(ui.ctx(), output.response.id);
    }

    if output.response.changed() {
        space_view.set_origin(ctx, &space_origin_string.clone().into());
    }

    if output.response.lost_focus() {
        if enter_key_hit {
            space_view.set_origin(ctx, &space_origin_string.clone().into());
        }
        keep_editing = false;
    }

    //
    // Display popup with suggestions
    //

    let popup_id = ui.make_persistent_id("suggestions");
    if output.response.has_focus() {
        ui.memory_mut(|mem| mem.open_popup(popup_id));
    }

    let suggestions_ui = |ui: &mut egui::Ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (idx, suggested_space_view) in filtered_space_view_suggestions.iter().enumerate() {
            let response = re_ui::list_item::ListItem::new(
                ctx.re_ui,
                suggested_space_view
                    .space_origin
                    .syntax_highlighted(ui.style()),
            )
            .force_hovered(*selected_suggestion == Some(idx))
            .show(ui);

            if response.hovered() {
                *selected_suggestion = None;
            }

            if response.clicked() {
                *space_origin_string = suggested_space_view.space_origin.to_string();
                space_view.set_origin(ctx, &space_origin_string.clone().into());
            }
        }

        let excluded_count = space_view_suggestions.len() - filtered_space_view_suggestions.len();
        if excluded_count > 0 {
            re_ui::list_item::ListItem::new(
                ctx.re_ui,
                format!("{excluded_count} hidden suggestions"),
            )
            .weak(true)
            .italics(true)
            .active(false)
            .show(ui);
        }
    };

    ReUi::list_item_popup(ui, popup_id, &output.response, 4.0, suggestions_ui);

    keep_editing
}
