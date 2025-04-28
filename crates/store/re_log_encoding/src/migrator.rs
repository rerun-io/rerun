use re_log_types::{external::re_tuid::Tuid, LogMsg};

use crate::legacy::*;

#[derive(thiserror::Error, Debug)]
pub enum MigrationError {
    #[error("{0}")]
    Custom(String),
}

impl LegacyLogMsg {
    pub fn migrate(self) -> Result<LogMsg, MigrationError> {
        match self {
            Self::SetStoreInfo(legacy_set_store_info) => {
                let LegacySetStoreInfo { row_id, info } = legacy_set_store_info;
                let LegacyStoreInfo {
                    application_id,
                    store_id,
                    cloned_from,
                    is_official_example,
                    started,
                } = info;

                Ok(LogMsg::SetStoreInfo(re_log_types::SetStoreInfo {
                    row_id: row_id.migrate(),
                    info: re_log_types::StoreInfo {
                        application_id,
                        store_id,
                        cloned_from,
                        store_source: re_log_types::StoreSource::Unknown,
                        store_version: None,
                    },
                }))
            }
            Self::ArrowMsg(store_id, arrow_msg) => {
                Ok(LogMsg::ArrowMsg(store_id, arrow_msg.migrate()))
            }
            Self::BlueprintActivationCommand(legacy_blueprint_activation_command) => Ok(
                LogMsg::BlueprintActivationCommand(legacy_blueprint_activation_command.migrate()),
            ),
        }
    }
}

impl LegacyTuid {
    fn migrate(&self) -> Tuid {
        Tuid::from_nanos_and_inc(self.time_ns, self.inc)
    }
}

impl LegacyArrowMsg {
    fn migrate(self) -> re_log_types::ArrowMsg {
        let Self {
            chunk_id,
            timepoint_max,
            batch,
        } = self;
        re_log_types::ArrowMsg {
            chunk_id: chunk_id.migrate(),
            timepoint_max,
            batch,
            on_release: None,
        }
    }
}

impl LegacyBlueprintActivationCommand {
    fn migrate(&self) -> re_log_types::BlueprintActivationCommand {
        unimplemented!()
    }
}
