use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

pub trait CommandExt {
    fn with_cwd<P>(self, cwd: P) -> Self
    where
        P: AsRef<Path>;

    fn with_arg<S>(self, arg: S) -> Self
    where
        S: AsRef<OsStr>;

    fn with_args<I, S>(self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>;

    fn with_env<K, V>(self, key: K, val: V) -> Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>;

    fn run_async(self) -> io::Result<()>;

    fn run(self) -> io::Result<()>;

    fn run_with_output(self) -> io::Result<Vec<u8>>;

    fn run_serde<T>(self) -> anyhow::Result<T>
    where
        T: for<'de> serde::Deserialize<'de>;
}

impl CommandExt for Command {
    #[inline]
    fn with_cwd<P>(mut self, cwd: P) -> Self
    where
        P: AsRef<Path>,
    {
        self.current_dir(cwd);
        self
    }

    #[inline]
    fn with_arg<S>(mut self, arg: S) -> Self
    where
        S: AsRef<OsStr>,
    {
        self.arg(arg);
        self
    }

    #[inline]
    fn with_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args(args);
        self
    }

    #[inline]
    fn with_env<K, V>(mut self, key: K, val: V) -> Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.env(key, val);
        self
    }

    fn run_async(mut self) -> io::Result<()> {
        self.spawn()?.wait_async()
    }

    fn run(mut self) -> io::Result<()> {
        self.spawn()?.wait()?.check()
    }

    fn run_with_output(mut self) -> io::Result<Vec<u8>> {
        let output = self
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?
            .wait_with_output()?;
        output.check()?;
        Ok(output.stdout)
    }

    fn run_serde<T>(self) -> anyhow::Result<T>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        let stdout = self.run_with_output()?;
        Ok(serde_json::from_slice(&stdout)?)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ExitCode(pub i32);

impl std::error::Error for ExitCode {}

impl std::fmt::Display for ExitCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "process exited with code {}", self.0)
    }
}

pub trait CheckStatus {
    fn check(&self) -> io::Result<()>;
}

impl CheckStatus for std::process::ExitStatus {
    fn check(&self) -> io::Result<()> {
        match self.success() {
            true => Ok(()),
            false => Err(io::Error::new(
                io::ErrorKind::Other,
                ExitCode(self.code().unwrap_or(-1)),
            )),
        }
    }
}

impl CheckStatus for std::process::Output {
    fn check(&self) -> io::Result<()> {
        self.status.check()
    }
}

pub trait WaitAsync {
    /// Wait with inherited IO
    fn wait_async(self) -> io::Result<()>;
}

impl WaitAsync for std::process::Child {
    fn wait_async(mut self) -> io::Result<()> {
        loop {
            if let Some(status) = self.try_wait()? {
                status.check()?;
                return Ok(());
            }
        }
    }
}
