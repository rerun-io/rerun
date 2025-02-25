use std::collections::HashSet;

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
        let mut verifier = Verifier::new()?;

        let Self { path_to_input_rrds } = self;

        // TODO(cmc): might want to make this configurable at some point.
        let version_policy = re_log_encoding::VersionPolicy::Warn;
        let (rx, _) = read_rrd_streams_from_file_or_stdin(version_policy, path_to_input_rrds);

        let mut log_msg_count = 0;
        for res in rx {
            verifier.verify_log_msg(res?);
            log_msg_count += 1;
        }

        if verifier.errors.is_empty() {
            eprintln!("{log_msg_count} chunks verified successfully.");
            Ok(())
        } else {
            for err in &verifier.errors {
                eprintln!("{err}");
            }
            Err(anyhow::anyhow!(
                "Verification failed with {} errors",
                verifier.errors.len()
            ))
        }
    }
}

struct Verifier {
    reflection: Reflection,
    errors: HashSet<String>,
}

impl Verifier {
    fn new() -> anyhow::Result<Self> {
        Ok(Self {
            reflection: re_types::reflection::generate_reflection()?,
            errors: HashSet::new(),
        })
    }

    fn verify_log_msg(&mut self, msg: LogMsg) {
        match msg {
            LogMsg::SetStoreInfo { .. } | LogMsg::BlueprintActivationCommand { .. } => {}

            LogMsg::ArrowMsg(_store_id, arrow_msg) => {
                self.verify_record_batch(&arrow_msg.batch);
            }
        }
    }

    fn verify_record_batch(&mut self, batch: &arrow::array::RecordBatch) {
        match re_sorbet::ChunkBatch::try_from(batch) {
            Ok(chunk_batch) => self.verify_chunk_batch(&chunk_batch),
            Err(err) => {
                self.errors.insert(format!("Failed to parse batch: {err}"));
            }
        }
    }

    fn verify_chunk_batch(&mut self, chunk_batch: &re_sorbet::ChunkBatch) {
        for (component_descriptor, column) in chunk_batch.component_columns() {
            let component_name = component_descriptor.component_name;

            if component_name.is_indicator_component() {
                continue; // Lacks reflection
            }

            if let Err(err) = self.verify_component_column(component_name, column) {
                self.errors.insert(format!(
                    "Failed to deserialize column {component_name:?}: {err}"
                ));
            }
        }
    }

    fn verify_component_column(
        &self,
        component_name: re_sdk::ComponentName,
        column: &dyn arrow::array::Array,
    ) -> anyhow::Result<()> {
        let component_reflection = self
            .reflection
            .components
            .get(&component_name)
            .ok_or_else(|| anyhow::anyhow!("Unknown component"))?;

        let list_array = column.as_list_opt::<i32>().ok_or_else(|| {
            anyhow::anyhow!("Expected list array, found {:?}", column.data_type())
        })?;

        assert_eq!(column.len() + 1, list_array.offsets().len());

        for i in 0..column.len() {
            let cell = list_array.value(i);
            (component_reflection.verify_arrow_array)(cell.as_ref())?;
        }

        Ok(())
    }
}
