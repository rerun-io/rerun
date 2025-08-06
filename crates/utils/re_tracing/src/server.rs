const PORT: u16 = puffin_http::DEFAULT_PORT;

/// Wraps a connection to a [`puffin`] viewer.
#[derive(Default)]
pub struct Profiler {
    server: Option<puffin_http::Server>,
}

impl Drop for Profiler {
    fn drop(&mut self) {
        // Commit the last stuff:
        puffin::GlobalProfiler::lock().new_frame();
    }
}

impl Profiler {
    pub fn start(&mut self) {
        puffin::set_scopes_on(true);
        crate::profile_function!();

        if self.server.is_none() {
            self.start_server();
        }
        start_puffin_viewer();
    }

    fn start_server(&mut self) {
        crate::profile_function!();
        let bind_addr = format!("0.0.0.0:{PORT}"); // Serve on all addresses.
        self.server = match puffin_http::Server::new(&bind_addr) {
            Ok(puffin_server) => {
                re_log::info!(
                    "Started puffin profiling server. View with:  cargo install puffin_viewer && puffin_viewer"
                );
                Some(puffin_server)
            }
            Err(err) => {
                re_log::warn!("Failed to start puffin profiling server: {err}");
                None
            }
        };
    }
}

fn start_puffin_viewer() {
    crate::profile_function!();
    let url = format!("127.0.0.1:{PORT}"); // Connect to localhost.
    let child = std::process::Command::new("puffin_viewer")
        .arg("--url")
        .arg(&url)
        .spawn();

    if let Err(err) = child {
        let cmd = format!("cargo install puffin_viewer && puffin_viewer --url {url}",);
        re_log::warn!("Failed to start puffin_viewer: {err}. Try connecting manually with:  {cmd}");

        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Info)
            .set_title("puffin_viewer required")
            .set_description(format!("To view the profiling data, run the following command:\n\n{cmd}\n\n(it has been copied to your clipboard)"))
            .show();
    }
}
