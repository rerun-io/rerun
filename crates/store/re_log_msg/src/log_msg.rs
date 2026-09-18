use std::sync::Arc;

use re_log_types::{StoreId, impl_into_enum};

use crate::{ArrowMsg, SetStoreInfo};

/// Command used for activating a blueprint once it has been fully transmitted.
///
/// This command serves two purposes:
/// - It is important that a blueprint is never activated before it has been fully
///   transmitted. Displaying, or allowing a user to modify, a half-transmitted
///   blueprint can cause confusion and bad interactions with the view heuristics.
/// - Additionally, this command allows fine-tuning the activation behavior itself
///   by specifying whether the blueprint should be immediately activated, or only
///   become the default for future activations.
#[derive(Clone, Debug, PartialEq, Eq, re_byte_size::SizeBytes)] // `PartialEq` used for tests in another crate
pub struct BlueprintActivationCommand {
    /// The blueprint this command refers to.
    pub blueprint_id: StoreId,

    /// Immediately make this the active blueprint for the associated `app_id`.
    ///
    /// Note that setting this to `false` does not mean the blueprint may not still end
    /// up becoming active. In particular, if `make_default` is true and there is no other
    /// currently active blueprint.
    pub make_active: bool,

    /// Make this the default blueprint for the `app_id`.
    ///
    /// The default blueprint will be used as the template when the user resets the
    /// blueprint for the app. It will also become the active blueprint if no other
    /// blueprint is currently active.
    pub make_default: bool,
}

impl BlueprintActivationCommand {
    /// Make `blueprint_id` the default blueprint for its associated `app_id`.
    pub fn make_default(blueprint_id: StoreId) -> Self {
        Self {
            blueprint_id,
            make_active: false,
            make_default: true,
        }
    }

    /// Immediately make `blueprint_id` the active blueprint for its associated `app_id`.
    ///
    /// This also sets `make_default` to true.
    pub fn make_active(blueprint_id: StoreId) -> Self {
        Self {
            blueprint_id,
            make_active: true,
            make_default: true,
        }
    }
}

/// The most general log message sent from the SDK to the server.
///
/// Note: this does not contain tables sent via [`crate::TableMsg`], as these concepts are fundamentally
/// different and should not be handled uniformly. For example, we don't want to store tables in
/// `.rrd` files.
#[must_use]
#[derive(Clone, Debug, PartialEq, re_byte_size::SizeBytes)] // `PartialEq` used for tests in another crate
// TODO(#8631): Remove `LogMsg`
pub enum LogMsg {
    /// A new recording has begun.
    ///
    /// Should usually be the first message sent.
    SetStoreInfo(SetStoreInfo),

    /// Log an entity using an [`ArrowMsg`].
    //
    // TODO(#6574): the store ID should be in the metadata here so we can remove the layer on top
    ArrowMsg(StoreId, ArrowMsg),

    /// Send after all messages in a blueprint to signal that the blueprint is complete.
    ///
    /// This is so that the viewer can wait with activating the blueprint until it is
    /// fully transmitted. Showing a half-transmitted blueprint can cause confusion,
    /// and also lead to problems with view heuristics.
    BlueprintActivationCommand(BlueprintActivationCommand),
}

impl LogMsg {
    pub fn store_id(&self) -> &StoreId {
        match self {
            Self::SetStoreInfo(msg) => &msg.info.store_id,
            Self::ArrowMsg(store_id, _) => store_id,
            Self::BlueprintActivationCommand(cmd) => &cmd.blueprint_id,
        }
    }

    pub fn set_store_id(&mut self, new_store_id: StoreId) {
        match self {
            Self::SetStoreInfo(store_info) => {
                store_info.info.store_id = new_store_id;
            }
            Self::ArrowMsg(store_id, _) => {
                *store_id = new_store_id;
            }
            Self::BlueprintActivationCommand(cmd) => {
                cmd.blueprint_id = new_store_id;
            }
        }
    }

    /// If we are an [`ArrowMsg`], records the current time as the moment the chunk passed
    /// `location`.
    pub fn track_latency(&mut self, location: re_sorbet::TimestampLocation) {
        if let Self::ArrowMsg(_, arrow_msg) = self {
            Arc::make_mut(&mut arrow_msg.batch).track_latency(location);
        }
    }
}

impl_into_enum!(SetStoreInfo, LogMsg, SetStoreInfo);
impl_into_enum!(
    BlueprintActivationCommand,
    LogMsg,
    BlueprintActivationCommand
);
