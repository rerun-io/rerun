use clap::{Arg, Command as ClapCommand};
use std::env;
use std::path::Path;
use std::process::{Command, ExitStatus};
use re_log;

/// Supported Rerun versions.
const SUPPORTED_VERSIONS: &[&str] = &["0.22.1", "0.17.0"];

fn main()
{
    let version = parse_args();
    println!("Parsed version: {}", version);
    validate_version(&version);
    prepare_x11();

    match run_docker(&version)
    {
        // success status
        Ok(status) if status.success() => {}

        // all other statuses
        Ok(_) =>
        {
            re_log::error!(name: "shutdown", "Docker container exited with an error.");
            std::process::exit(1);
        }

        // error
        Err(err) =>
        {
            re_log::error!(name: "shutdown", "Failed to launch Docker container: {}", err);
            std::process::exit(1);
        }
    }
}

/// Parses CLI arguments and returns the selected version.
fn parse_args() -> String
{
    let matches = ClapCommand::new("rerun_docker")
        .about("Launches rerun inside a Docker container with X11 and GPU support")
        .arg(
            Arg::new("version")
                .help(format!(
                    "rerun version to run (supported: {})",
                    SUPPORTED_VERSIONS.join(", ")
                ))
                .required(true),
        )
        .get_matches();

    match matches.get_one::<String>("version") {
        Some(version) => version.to_owned(),
        None => {
            re_log::error!("Version argument is required");
            std::process::exit(1);
        }
    }
}


/// Validates that the provided version is supported.
fn validate_version(version: &str)
{
    if !SUPPORTED_VERSIONS.contains(&version)
    {

        re_log::info!("Unsupported version '{}'.", version);
        re_log::info!("    Supported versions are: {}", SUPPORTED_VERSIONS.join(", "));
        std::process::exit(1);

    }
}

/// Prepares X11 forwarding for rerun:VERSION docker container.
fn prepare_x11()
{
    let xauth_path = "/tmp/.docker.xauth";
    let display = env::var("DISPLAY").unwrap_or_else(|_| ":0".into());

    re_log::info!("[+] Preparing X11 forwarding for Docker...");

    if !Path::new(xauth_path).exists()
    {

        run_cmd("touch", &[xauth_path]);

        let cmd = format!(
            "xauth nlist {} | sed -e 's/^..../ffff/' | xauth -f {} nmerge -",
            display, xauth_path
        );

        run_cmd("sh", &["-c", &cmd]);
        run_cmd("chmod", &["777", xauth_path]);

    }
}

/// Executes the Docker command to launch the rerun container.
fn run_docker(version: &str) -> std::io::Result<ExitStatus>
{
    let display = env::var("DISPLAY").unwrap_or_else(|_| ":0".into());
    let xsock = "/tmp/.X11-unix";
    let xauth = "/tmp/.docker.xauth";

    re_log::info!("[+] Starting rerun:{} in Docker container...", version);

    Command::new("docker")
        .args([
            "run",
            "--runtime=nvidia",
            "--rm",
            "--gpus",
            "all",
            "-it",
            "--privileged",
            "--network=host",
            "-e",
            "NVIDIA_DRIVER_CAPABILITIES=all",
            "-e",
            &format!("DISPLAY={}", display),
            "-v",
            &format!("{}:{}", xsock, xsock),
            "-v",
            &format!("{}:{}", xauth, xauth),
            "-e",
            &format!("XAUTHORITY={}", xauth),
            &format!("rerun:{}", version),
            "rerun",
        ])
        .status()
}

/// Utility wrapper to execute a command and exit on failure.
fn run_cmd(cmd: &str, args: &[&str])
{
    let status = Command::new(cmd)
        .args(args)
        .status()
        .unwrap_or_else(|err|
        {
            re_log::error!(name: "command_failed", "Failed to execute '{}': {}", cmd, err);
            std::process::exit(1);
        });

    if !status.success()
    {
        re_log::error!(name: "shutdown", "Command '{}' exited with non-zero status.", cmd);
        std::process::exit(1);
    }
}
