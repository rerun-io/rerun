//! [`Operation`]: every viewer-control operation, and what calling each one can do.

use re_agent_tools::ToolEffect;

use crate::ScreenshotDelivery;

/// Every viewer-control operation: one variant per field of the `ViewerControlRequest.kind`
/// oneof in `viewer_control.proto`, named the same in `snake_case`.
///
/// The test `the_operations_are_the_fields_of_the_oneof` keeps the two in step, so adding an
/// operation to the `.proto` means adding it here, and deciding its [`Self::effect`] and
/// [`Self::may_be_open_world`].
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, strum::EnumString, strum::IntoStaticStr, strum::VariantArray,
)]
#[strum(serialize_all = "snake_case")]
pub enum Operation {
    CaptureProfileTrace,
    Close,
    DescribeCommands,
    GetBlueprint,
    GetDatasetAssetsSchema,
    GetDatasetSchema,
    GetLocation,
    GetLogs,
    GetRecordingSchema,
    GetSelection,
    GetSettings,
    GetState,
    GetTableSchema,
    GetTransformGraph,
    HighlightRect,
    ListCommands,
    ListViewTypes,
    Open,
    RunCommand,
    SaveScreenshot,
    SetBlueprint,
    SetLocation,
    SetSettings,
    SetTimeCursor,
    SubmitFeedback,
}

impl Operation {
    /// The operation spelled `name` in the `.proto`, e.g. `get_state`.
    pub fn from_name(name: &str) -> Option<Self> {
        name.parse().ok()
    }

    /// The name of the operation in the `.proto`, e.g. `get_state`.
    pub fn name(self) -> &'static str {
        self.into()
    }

    /// What calling the operation can change, when screenshots are delivered as `screenshot`.
    ///
    /// `ReadOnly` and `NonDestructive` operations run without asking the user. Everything else
    /// asks first: `open` loads data from any URL, `close` drops recordings,
    /// `capture_profile_trace` writes files, `set_settings` changes saved settings, and
    /// `submit_feedback` sends text to Rerun. `run_command` asks too, since it runs any UI
    /// command, but a host that sees the arguments can let the undoable commands through.
    pub fn effect(self, screenshot: ScreenshotDelivery) -> ToolEffect {
        match self {
            Self::DescribeCommands
            | Self::GetBlueprint
            | Self::GetDatasetAssetsSchema
            | Self::GetDatasetSchema
            | Self::GetLocation
            | Self::GetLogs
            | Self::GetRecordingSchema
            | Self::GetSelection
            | Self::GetSettings
            | Self::GetState
            | Self::GetTableSchema
            | Self::GetTransformGraph
            | Self::ListCommands
            | Self::ListViewTypes => ToolEffect::ReadOnly,

            // Writing the image to a file may overwrite another one.
            Self::SaveScreenshot => match screenshot {
                ScreenshotDelivery::Inline => ToolEffect::ReadOnly,
                ScreenshotDelivery::File => ToolEffect::Destructive,
            },

            Self::HighlightRect | Self::SetBlueprint | Self::SetLocation | Self::SetTimeCursor => {
                ToolEffect::NonDestructive
            }

            Self::CaptureProfileTrace
            | Self::Close
            | Self::Open
            | Self::RunCommand
            | Self::SetSettings
            | Self::SubmitFeedback => ToolEffect::Destructive,
        }
    }

    /// Whether calling the operation may reach beyond the viewer and this machine's files. See
    /// [`re_agent_tools::ToolDefinition::open_world`].
    pub fn may_be_open_world(self) -> bool {
        match self {
            // The schema operations read from a catalog server, `open` loads data from any URL,
            // `run_command` runs any UI command, and `submit_feedback` sends text to Rerun.
            Self::GetDatasetAssetsSchema
            | Self::GetDatasetSchema
            | Self::Open
            | Self::RunCommand
            | Self::SubmitFeedback => true,

            Self::CaptureProfileTrace
            | Self::Close
            | Self::DescribeCommands
            | Self::GetBlueprint
            | Self::GetLocation
            | Self::GetLogs
            | Self::GetRecordingSchema
            | Self::GetSelection
            | Self::GetSettings
            | Self::GetState
            | Self::GetTableSchema
            | Self::GetTransformGraph
            | Self::HighlightRect
            | Self::ListCommands
            | Self::ListViewTypes
            | Self::SaveScreenshot
            | Self::SetBlueprint
            | Self::SetLocation
            | Self::SetSettings
            | Self::SetTimeCursor => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_operations_are_the_fields_of_the_oneof() {
        use strum::VariantArray as _;

        let fields: Vec<String> = crate::operation_oneof()
            .fields()
            .map(|field| field.name().to_owned())
            .collect();
        for field in &fields {
            assert!(Operation::from_name(field).is_some(), "{field} is missing");
        }
        for operation in Operation::VARIANTS {
            assert!(
                fields.iter().any(|field| field == operation.name()),
                "{operation:?} is not in the .proto"
            );
        }
    }

    #[test]
    fn only_operations_that_may_leave_the_machine_are_open_world() {
        for (operation, expected) in [
            (Operation::Close, false),
            (Operation::GetDatasetSchema, true),
            (Operation::GetState, false),
            (Operation::GetTableSchema, false),
            (Operation::Open, true),
            (Operation::RunCommand, true),
            (Operation::SetTimeCursor, false),
            (Operation::SubmitFeedback, true),
        ] {
            assert_eq!(operation.may_be_open_world(), expected, "{operation:?}");
        }
    }
}
