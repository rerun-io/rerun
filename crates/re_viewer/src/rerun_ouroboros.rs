// TODO(emilk): enable this from the web viewer too.
#[cfg(all(feature = "re_sdk", not(target_arch = "wasm32")))]
mod implementation {
    use std::sync::Arc;

    use re_sdk::Session;

    /// Rerun biting its own tail: Rerun sending log messages to another Rerun.
    pub struct RerunOuroboros {
        recording_info: re_sdk::RecordingInfo,
        session: Option<Arc<Session>>,
    }

    impl Default for RerunOuroboros {
        fn default() -> Self {
            Self::new()
        }
    }

    impl RerunOuroboros {
        pub const SUPPORTED: bool = true;

        fn new() -> Self {
            let (rerun_enabled, recording_info) = re_sdk::SessionBuilder::new("rerun_viewer")
                .default_enabled(false)
                .finalize();

            let mut slf = Self {
                recording_info,
                session: None,
            };

            if rerun_enabled {
                slf.start();
            }

            slf
        }

        pub fn start(&mut self) {
            start_rerun_viewer();

            if self.session.is_none() {
                let sink = Box::new(re_sdk::sink::TcpSink::new(re_sdk::default_server_addr()));
                let session = Arc::new(Session::new(self.recording_info.clone(), sink));

                #[cfg(not(target_arch = "wasm32"))]
                spawn_memory_monitor(session.clone());

                self.session = Some(session);
            }
        }
    }

    fn start_rerun_viewer() {
        let child = std::process::Command::new("rerun")
            .arg("--memory-limit")
            .arg("2GB")
            .spawn();

        if let Err(err) = child {
            let cmd = "cargo install rerun && rerun --memory-limit 2GB";
            crate::misc::Clipboard::with(|cliboard| cliboard.set_text(cmd.to_owned()));
            re_log::warn!("Failed to start rerun: {err}. Try connecting manually with:  {cmd}");

            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Info)
                .set_title("rerun required")
                .set_description(&format!("To view the Rerun data from the Rerun Viewer, run the following command:\n\n{cmd}\n\n(it has been copied to your clipboard)"))
                .show();
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn spawn_memory_monitor(session: Arc<Session>) {
        if let Err(err) = std::thread::Builder::new()
            .name("rerun_memory_monitor".to_owned())
            .spawn(move || loop {
                let memory_use = re_memory::MemoryUse::capture();
                let re_memory::MemoryUse { resident, counted } = memory_use;
                if let Some(resident) = resident {
                    session.log_scalar("memory/resident", resident as _);
                }
                if let Some(counted) = counted {
                    session.log_scalar("memory/counted", counted as _);
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            })
        {
            re_log::warn!("Failed to spawn memory monitor: {err}");
        }
    }
}

#[cfg(not(all(feature = "re_sdk", not(target_arch = "wasm32"))))]
mod implementation {
    #[derive(Default)]
    pub struct RerunOuroboros {}

    impl RerunOuroboros {
        pub const SUPPORTED: bool = false;

        pub fn start(&self) {}
    }
}

pub use implementation::*;
