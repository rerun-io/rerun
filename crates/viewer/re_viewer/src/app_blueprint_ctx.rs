use re_entity_db::EntityDb;
use re_viewer_context::{BlueprintContext, CommandSender};

pub struct AppBlueprintCtx<'a> {
    pub command_sender: &'a CommandSender,
    pub current_blueprint: &'a EntityDb,
    pub default_blueprint: Option<&'a EntityDb>,
    pub blueprint_query: re_chunk::LatestAtQuery,
}

impl BlueprintContext for AppBlueprintCtx<'_> {
    fn command_sender(&self) -> &CommandSender {
        self.command_sender
    }

    fn current_blueprint(&self) -> &EntityDb {
        self.current_blueprint
    }

    fn default_blueprint(&self) -> Option<&EntityDb> {
        self.default_blueprint
    }

    fn blueprint_query(&self) -> &re_chunk::LatestAtQuery {
        &self.blueprint_query
    }
}
