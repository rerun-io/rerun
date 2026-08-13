//! Which viewer the integration tests drive, and how, read from the environment.

use std::path::PathBuf;
use std::time::Duration;

/// Which viewer an [`InspectionHarness`](super::InspectionHarness) drives, from
/// `RERUN_INTEGRATION_TEST_TARGET`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TargetViewer {
    /// Run the viewer in-process via an [`egui_kittest::Harness`], servicing inspection requests
    /// against an [`InspectionPlugin`](egui_inspection::InspectionPlugin). No subprocess or
    /// prebuilt binary needed.
    #[default]
    InProcess,

    /// Launch a native `rerun --headless` process and drive it over gRPC.
    Cli,

    /// Run the real wasm web viewer in a browser (headless Chrome by default) and drive it over
    /// the Chrome `DevTools` Protocol. Requires building with `--features browser`, a prebuilt web
    /// viewer (`pixi run rerun-build-web`), and Chrome.
    Browser,
}

/// Environment configuration for the integration tests, read once per test process.
///
/// Every knob here is a `RERUN_INTEGRATION_TEST_*` environment variable; see [`TestEnv::from_env`].
#[derive(Clone, Debug, Default)]
pub struct TestEnv {
    /// Which viewer to drive: `RERUN_INTEGRATION_TEST_TARGET`.
    pub target: TargetViewer,

    /// Run the viewer in a visible window rather than headless, so a developer can watch the test:
    /// `RERUN_INTEGRATION_TEST_WINDOWED`.
    pub windowed: bool,

    /// An artificial delay applied after every inspection command, so a developer watching a
    /// windowed viewer can follow what each step does: `RERUN_INTEGRATION_TEST_DELAY`, in
    /// milliseconds (e.g. `500`).
    pub command_delay: Option<Duration>,

    /// Override for the `rerun` binary launched by [`TargetViewer::Cli`]:
    /// `RERUN_INTEGRATION_TEST_BIN`.
    pub rerun_binary: Option<PathBuf>,

    /// Override for the built web viewer directory served to [`TargetViewer::Browser`]:
    /// `RERUN_INTEGRATION_TEST_WEB_VIEWER`.
    #[cfg(feature = "browser")]
    pub web_viewer_dir: Option<PathBuf>,
}

impl TestEnv {
    /// The configuration for this test process, parsed from the environment on first use.
    pub fn get() -> &'static Self {
        static ENV: std::sync::LazyLock<TestEnv> = std::sync::LazyLock::new(TestEnv::from_env);
        &ENV
    }

    /// Parse the `RERUN_INTEGRATION_TEST_*` environment variables, panicking on invalid values.
    fn from_env() -> Self {
        let target = match std::env::var("RERUN_INTEGRATION_TEST_TARGET")
            .ok()
            .as_deref()
        {
            None | Some("" | "in-process") => TargetViewer::InProcess,
            Some("cli") => TargetViewer::Cli,
            Some("browser") => TargetViewer::Browser,
            Some(other) => panic!(
                "Unknown RERUN_INTEGRATION_TEST_TARGET {other:?} \
                 (expected `in-process`, `cli`, or `browser`)"
            ),
        };

        let command_delay = match std::env::var("RERUN_INTEGRATION_TEST_DELAY") {
            Ok(delay) => {
                let ms = delay.parse::<u64>().unwrap_or_else(|err| {
                    panic!("Invalid RERUN_INTEGRATION_TEST_DELAY {delay:?}: {err}")
                });
                (ms > 0).then(|| Duration::from_millis(ms))
            }
            Err(_) => None,
        };

        Self {
            target,
            windowed: re_log::env_var_is_truthy("RERUN_INTEGRATION_TEST_WINDOWED"),
            command_delay,
            rerun_binary: std::env::var("RERUN_INTEGRATION_TEST_BIN")
                .ok()
                .map(PathBuf::from),
            #[cfg(feature = "browser")]
            web_viewer_dir: std::env::var("RERUN_INTEGRATION_TEST_WEB_VIEWER")
                .ok()
                .map(PathBuf::from),
        }
    }

    /// Locate the `rerun` binary to launch.
    ///
    /// `re_integration_test` isn't the `rerun-cli` package, so `CARGO_BIN_EXE_rerun` isn't set. We
    /// use [`Self::rerun_binary`] if set, and otherwise look next to the test executable (i.e.
    /// `target/<profile>/rerun`).
    pub(super) fn resolve_rerun_binary(&self) -> PathBuf {
        if let Some(path) = &self.rerun_binary {
            return path.clone();
        }

        let test_exe = std::env::current_exe().expect("Failed to get current test executable path");
        // The test executable lives in `target/<profile>/deps/`; the `rerun` binary is one level up.
        let mut dir = test_exe
            .parent()
            .expect("test executable has no parent directory")
            .to_path_buf();
        if dir.ends_with("deps") {
            dir.pop();
        }
        let binary = dir.join(format!("rerun{}", std::env::consts::EXE_SUFFIX));

        assert!(
            binary.exists(),
            "Could not find the `rerun` binary at {}.\n\
             Build it with `pixi run rerun-build` (or `cargo build -p rerun-cli --bin rerun`), \
             or set RERUN_INTEGRATION_TEST_BIN to its path.",
            binary.display()
        );
        binary
    }

    /// Locate the built web viewer directory (containing `re_viewer_bg.wasm`, `re_viewer.js`,
    /// `index.html`, …), either from [`Self::web_viewer_dir`] or relative to this crate.
    #[cfg(feature = "browser")]
    pub(super) fn resolve_web_viewer_dir(&self) -> PathBuf {
        if let Some(path) = &self.web_viewer_dir {
            return path.clone();
        }

        // <workspace>/crates/viewer/re_web_viewer_server/web_viewer, relative to this crate.
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../crates/viewer/re_web_viewer_server/web_viewer");

        assert!(
            dir.join("re_viewer_bg.wasm").exists(),
            "Could not find the built web viewer at {}.\n\
             Build it with `pixi run rerun-build-web`, or set RERUN_INTEGRATION_TEST_WEB_VIEWER \
             to its directory.",
            dir.display()
        );
        dir
    }
}
