use std::sync::LazyLock;

use parking_lot::Mutex;

const PORT_CPU: u16 = puffin_http::DEFAULT_PORT;
const PORT_GPU: u16 = puffin_http::DEFAULT_PORT + 1;

// Since the timing information we get from WGPU may be several frames behind the CPU, we can't report these frames to
// the singleton returned by `puffin::GlobalProfiler::lock`. Instead, we need our own `puffin::GlobalProfiler` that we
// can be several frames behind puffin's main global profiler singleton.
static PUFFIN_GPU_PROFILER: LazyLock<Mutex<puffin::GlobalProfiler>> =
    LazyLock::new(|| Mutex::new(puffin::GlobalProfiler::default()));

/// Wraps a connection to a [`puffin`] viewer.
#[derive(Default)]
pub struct Profiler {
    server_cpu: Option<puffin_http::Server>,
    server_gpu: Option<puffin_http::Server>,
}

impl Drop for Profiler {
    fn drop(&mut self) {
        // Commit the last stuff:
        puffin::GlobalProfiler::lock().new_frame();
    }
}

impl Profiler {
    pub fn start_cpu(&mut self) {
        puffin::set_scopes_on(true);
        crate::profile_function!();

        if self.server_cpu.is_none() {
            self.server_cpu = start_puffin_server_cpu();
        }
        start_puffin_viewer(PORT_CPU);
    }

    pub fn start_gpu(&mut self) {
        crate::profile_function!();

        if self.server_gpu.is_none() {
            self.server_gpu = start_puffin_server_gpu();
        }
        start_puffin_viewer(PORT_GPU);
    }

    #[expect(clippy::unused_self)]
    pub fn puffin_gpu_profiler(&self) -> &Mutex<puffin::GlobalProfiler> {
        &PUFFIN_GPU_PROFILER
    }

    pub fn is_gpu_profiler_server_active(&self) -> bool {
        self.server_gpu.is_some()
    }
}

fn start_puffin_server_cpu() -> Option<puffin_http::Server> {
    crate::profile_function!();

    let bind_addr = format!("0.0.0.0:{PORT_CPU}"); // Serve on all addresses.
    handle_server_result(puffin_http::Server::new(&bind_addr))
}

fn start_puffin_server_gpu() -> Option<puffin_http::Server> {
    crate::profile_function!();

    let bind_addr = format!("0.0.0.0:{PORT_GPU}"); // Serve on all addresses.

    handle_server_result(puffin_http::Server::new_custom(
        &bind_addr,
        |sink| PUFFIN_GPU_PROFILER.lock().add_sink(sink),
        |id| _ = PUFFIN_GPU_PROFILER.lock().remove_sink(id),
    ))
}

fn handle_server_result(
    server_result: anyhow::Result<puffin_http::Server>,
) -> Option<puffin_http::Server> {
    match server_result {
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
    }
}

fn start_puffin_viewer(port: u16) {
    crate::profile_function!();
    let url = format!("127.0.0.1:{port}"); // Connect to localhost.
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
