use super::Channel;
use crate::build_examples::wait_for_output::wait_for_output;
use indicatif::MultiProgress;
use std::process::Command;

/// Install the selected examples in the current environment.
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "install")]
pub struct Install {
    #[argh(option, description = "include only examples in this channel")]
    channel: Channel,

    #[argh(option, description = "run only these examples")]
    examples: Vec<String>,
}

impl Install {
    pub fn run(self) -> anyhow::Result<()> {
        let workspace_root = re_build_tools::cargo_metadata()?.workspace_root;
        let mut examples = if self.examples.is_empty() {
            self.channel.examples(workspace_root)?
        } else {
            Channel::Nightly
                .examples(workspace_root)?
                .into_iter()
                .filter(|example| self.examples.contains(&example.name))
                .collect()
        };
        examples.sort_by(|a, b| a.name.cmp(&b.name));

        let mut cmd = Command::new("python3");
        cmd.arg("-m").arg("pip").arg("install");

        for example in &examples {
            cmd.arg("-e").arg(&example.dir);
        }

        let progress = MultiProgress::new();
        wait_for_output(cmd, "installing examples", &progress)?;

        println!("Successfully installed examples");

        Ok(())
    }
}
