use std::{collections::HashMap, env::Args};

use re_error::format;

// ---

// TODO:
// - use it in rust
// - use it in python!

// TODO
pub type ArgsRest = Vec<String>;

#[derive(thiserror::Error, Debug)]
pub enum RerunArgsError {
    // Batches
    #[error("Missing parameter for {0}")]
    MissingParameter(String),
}

#[derive(Debug, Clone, Default)]
pub struct RerunArgs {
    /// Entirely disables the SDK, turning every call into no-ops.
    // TODO(cmc): or maybe the other way? :/
    pub disabled: bool,

    /// Log the data by sending it to the given address over TCP.
    pub connect_to: Option<String>,

    /// Save the logged data to an `rrd` file on disk.
    pub save_to: Option<String>,

    // TODO: doc
    pub serve_on: Option<String>,
}

impl RerunArgs {
    // TODO: doc
    pub fn from_env(env: impl IntoIterator<Item = (String, String)>) -> Self {
        let mut env: HashMap<_, _> = env.into_iter().collect();
        Self {
            disabled: env.remove("RERUN_SDK_DISABLED").is_some(),
            connect_to: env.remove("RERUN_SDK_CONNECT"),
            save_to: env.remove("RERUN_SDK_SAVE"),
            serve_on: env.remove("RERUN_SDK_SERVE"),
        }
    }

    // TODO: doc
    pub fn from_args(
        args: impl IntoIterator<Item = String>,
        prefix: Option<&str>,
    ) -> Result<(Self, ArgsRest), RerunArgsError> {
        let mut rest = Vec::new();
        let mut rr_args = RerunArgs::default();

        let prefix = prefix.unwrap_or_default();
        let disabled = format!("--{prefix}disabled");
        let connect = format!("--{prefix}connect");
        let save = format!("--{prefix}save");
        let serve = format!("--{prefix}serve");

        let mut args = args.into_iter().peekable();
        while let Some(arg) = args.next() {
            if arg == disabled {
                rr_args.disabled = true;
            } else if arg == connect {
                let addr = match args.peek() {
                    Some(addr) if !addr.starts_with('-') => args.next().unwrap(),
                    Some(_) => return Err(RerunArgsError::MissingParameter(arg)),
                    None => "127.0.0.1:9876".to_owned(), // TODO: constant somewhere?
                };
                rr_args.connect_to = addr.into();
            } else if arg == save {
                let path = match args.peek() {
                    Some(path) if !path.starts_with('-') => args.next().unwrap(),
                    _ => return Err(RerunArgsError::MissingParameter(arg)),
                };
                rr_args.save_to = path.into();
            } else if arg == serve {
                let addr = match args.peek() {
                    Some(addr) if !addr.starts_with('-') => args.next().unwrap(),
                    Some(_) => return Err(RerunArgsError::MissingParameter(arg)),
                    None => "0.0.0.0:9090".to_owned(), // TODO: constant somewhere?
                };
                rr_args.serve_on = addr.into();
            } else {
                rest.push(arg);
            }
        }

        Ok((rr_args, rest))
    }

    pub fn from_args_then_env(
        args: impl IntoIterator<Item = String>,
        prefix: Option<&str>,
        env: impl IntoIterator<Item = (String, String)>,
    ) -> Result<(Self, ArgsRest), RerunArgsError> {
        let (rr_args1, args_rest) = Self::from_args(args, prefix)?;
        let rr_args2 = Self::from_env(env);

        let rr_args = Self {
            disabled: rr_args1.disabled || rr_args2.disabled,
            connect_to: rr_args1.connect_to.or(rr_args2.connect_to),
            save_to: rr_args1.save_to.or(rr_args2.save_to),
            serve_on: rr_args1.serve_on.or(rr_args2.serve_on),
        };

        Ok((rr_args, args_rest))
    }
}

// TODO: tests
