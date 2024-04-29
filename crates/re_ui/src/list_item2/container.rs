use once_cell::sync::Lazy;

#[derive(Debug, Clone)]
pub struct State {
    pub(crate) background_x_range: egui::Rangef,
    //stuff for dual column
}

impl Default for State {
    fn default() -> Self {
        Self {
            background_x_range: egui::Rangef::NOTHING,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StateStack(Vec<State>);

static STATE_STACK_ID: Lazy<egui::Id> = Lazy::new(|| egui::Id::new("re_ui_list_item_state_stack"));

impl StateStack {
    pub(crate) fn push(ctx: &egui::Context, state: State) {
        ctx.data_mut(|writer| {
            let stack: &mut StateStack = writer.get_temp_mut_or_default(*STATE_STACK_ID);
            stack.0.push(state);
        });
    }

    pub(crate) fn pop(ctx: &egui::Context) -> Option<State> {
        ctx.data_mut(|writer| {
            let stack: &mut StateStack = writer.get_temp_mut_or_default(*STATE_STACK_ID);
            stack.0.pop()
        })
    }

    pub(crate) fn top(ctx: &egui::Context) -> State {
        ctx.data_mut(|writer| {
            let stack: &mut StateStack = writer.get_temp_mut_or_default(*STATE_STACK_ID);
            let state = stack.0.last();
            if state.is_none() {
                re_log::warn_once!(
                    "Attempted to access empty ListItem state stack, returning default state"
                );
            }
            state.cloned().unwrap_or_default()
        })
    }
}

#[derive(Debug, Clone)]
pub struct ListItemContainer {
    background_x_range: egui::Rangef,
}

impl ListItemContainer {
    pub fn new(background_x_range: egui::Rangef) -> Self {
        Self { background_x_range }
    }

    pub fn ui<R>(
        &self,
        ui: &mut egui::Ui,
        id: impl Into<egui::Id>,
        content: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        /*
        data contains two set of things:
        - some per container state
        - a global state stack that is read by actual list items
         */

        let id = id.into();

        // read the state for this container, if any
        let state: Option<State> = ui.data(|reader| reader.get_temp(id));
        let mut state = state.unwrap_or_default();

        // always overwrite the background range
        state.background_x_range = self.background_x_range;

        // push the state to the state stack
        StateStack::push(ui.ctx(), state.clone());
        let result = content(ui);
        let state = StateStack::pop(ui.ctx());

        // save the state for this container
        if let Some(state) = state {
            ui.data_mut(|writer| writer.insert_temp(id, state));
        }

        result
    }
}
