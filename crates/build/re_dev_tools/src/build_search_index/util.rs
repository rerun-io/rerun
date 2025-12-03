use std::borrow::Cow;
use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

use indicatif::ProgressBar;

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

    #[expect(unused)]
    fn with_env<K, V>(self, key: K, val: V) -> Self
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>;

    #[expect(unused)]
    fn run(self) -> io::Result<()>;

    fn output(self) -> anyhow::Result<Vec<u8>>;

    fn parse_json<T>(self) -> anyhow::Result<T>
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

    fn run(mut self) -> io::Result<()> {
        self.spawn()?.wait()?.check()
    }

    fn output(mut self) -> anyhow::Result<Vec<u8>> {
        let output = self
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?
            .wait_with_output()?;
        if let Err(err) = output.check() {
            anyhow::bail!(
                "failed to run {self:?}\n{err}\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(output.stdout)
    }

    fn parse_json<T>(self) -> anyhow::Result<T>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        let stdout = self.output()?;
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
        if self.success() {
            Ok(())
        } else {
            Err(io::Error::other(ExitCode(self.code().unwrap_or(-1))))
        }
    }
}

impl CheckStatus for std::process::Output {
    fn check(&self) -> io::Result<()> {
        self.status.check()
    }
}

pub trait ProgressBarExt {
    fn set(&self, message: impl Into<Cow<'static, str>>, is_tty: bool);
}

impl ProgressBarExt for ProgressBar {
    fn set(&self, message: impl Into<Cow<'static, str>>, is_tty: bool) {
        // `indicatif` doesn't print _anything_ when stdout is not a tty,
        // which makes it harder to diagnose issues on CI.
        // https://github.com/console-rs/indicatif/issues/87
        if is_tty {
            self.println(self.message());
            self.set_message(message);
        } else {
            let message = message.into();
            println!("{message}");
        }
    }
}
