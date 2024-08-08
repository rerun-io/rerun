/// Where are we calling [`run`] from?
// TODO(jleibs): Maybe remove call-source all together.
// However, this context of spawn vs direct CLI-invocation still seems
// useful for analytics. We just need to capture the data some other way.
pub enum CallSource {
    /// Called from a command-line-input (the terminal).
    Cli,
}

impl CallSource {
    #[cfg(feature = "native_viewer")]
    fn app_env(&self) -> re_viewer::AppEnvironment {
        match self {
            Self::Cli => re_viewer::AppEnvironment::RerunCli {
                rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
                llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
            },
        }
    }
}

// ---

mod entrypoint;
mod rrd;
mod stdio;

#[cfg(feature = "analytics")]
mod analytics;

pub use self::entrypoint::run;
pub use self::rrd::RrdCommands;
pub use self::stdio::read_rrd_streams_from_file_or_stdin;

#[cfg(feature = "analytics")]
pub(crate) use self::analytics::AnalyticsCommands;
