use anyhow::Context as _;

use arrow::array::AsArray;
use re_log_types::LogMsg;
use re_types::reflection::Reflection;

use crate::commands::read_rrd_streams_from_file_or_stdin;

// ---

#[derive(Debug, Clone, clap::Parser)]
pub struct VerifyCommand {
    /// Paths to read from. Reads from standard input if none are specified.
    path_to_input_rrds: Vec<String>,
}

impl VerifyCommand {
    pub fn run(&self) -> anyhow::Result<()> {
        let reflection = re_types::reflection::generate_reflection()?;

        let Self { path_to_input_rrds } = self;

        // TODO(cmc): might want to make this configurable at some point.
        let version_policy = re_log_encoding::VersionPolicy::Warn;
        let (rx, _) = read_rrd_streams_from_file_or_stdin(version_policy, path_to_input_rrds);

        let mut log_msg_count = 0;
        for res in rx {
            verify_log_msg(&reflection, res?)?;
            log_msg_count += 1;
        }

        eprintln!("{log_msg_count} chunks verified successfully.");

        Ok(())
    }
}

fn verify_log_msg(reflection: &Reflection, msg: LogMsg) -> anyhow::Result<()> {
    match msg {
        LogMsg::SetStoreInfo { .. } | LogMsg::BlueprintActivationCommand { .. } => {}

        LogMsg::ArrowMsg(_store_id, arrow_msg) => {
            verify_record_batch(reflection, &arrow_msg.batch)?;
        }
    }
    Ok(())
}

fn verify_record_batch(
    reflection: &Reflection,
    batch: &arrow::array::RecordBatch,
) -> anyhow::Result<()> {
    let chunk_batch = re_sorbet::ChunkBatch::try_from(batch)?;

    for (component_descriptor, column) in chunk_batch.component_columns() {
        let component_name = component_descriptor.component_name;

        if component_name.is_indicator_component() {
            continue; // Lacks reflection
        }

        let component_reflection = reflection
            .components
            .get(&component_name)
            .ok_or_else(|| anyhow::anyhow!("Unknown component: {component_name:?}"))?;

        let list_array = column.as_list_opt::<i32>().ok_or_else(|| {
            anyhow::anyhow!(
                "Expected list array, found {:?} (ComponentName: {component_name:?})",
                column.data_type()
            )
        })?;

        assert_eq!(column.len() + 1, list_array.offsets().len());

        for i in 0..column.len() {
            let cell = list_array.value(i);
            (component_reflection.verify_arrow_array)(cell.as_ref())
                .with_context(|| format!("ComponentName: {component_name:?}"))?;
        }
    }

    Ok(())
}
