use re_log_types::{DataRow, EntityPath, RowId, TimePoint};

use crate::{SystemCommand, SystemCommandSender as _, ViewerContext};

impl ViewerContext<'_> {
    /// Helper to save a component to the blueprint store.
    pub fn save_blueprint_component<'a, C>(&self, entity_path: &EntityPath, component: C)
    where
        C: re_types::Component + Clone + 'a,
        std::borrow::Cow<'a, C>: std::convert::From<C>,
    {
        let timepoint = TimePoint::timeless();

        let row = DataRow::from_cells1_sized(
            RowId::new(),
            entity_path.clone(),
            timepoint.clone(),
            1,
            [component],
        )
        .unwrap(); // TODO(emilk): statically check that the component is a mono-component - then this cannot fail!

        self.command_sender
            .send_system(SystemCommand::UpdateBlueprint(
                self.store_context.blueprint.store_id().clone(),
                vec![row],
            ));
    }
}
