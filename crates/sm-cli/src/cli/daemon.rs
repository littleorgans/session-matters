use std::fs::{self, OpenOptions};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use sm_core::{DaemonStatus, RpcRequest, SmPaths};

use crate::cli::cli_def::{DaemonAction, DaemonArgs};

pub async fn run(args: DaemonArgs) -> Result<()> {
    let paths = SmPaths::from_env()?;
    match args.action {
        DaemonAction::Start => start(&paths),
        DaemonAction::Stop => stop(&paths).await,
        DaemonAction::Status => {
            print_status(&status(&paths));
            Ok(())
        }
    }
}

fn start(paths: &SmPaths) -> Result<()> {
    fs::create_dir_all(&paths.dir).context("failed to create runtime directory")?;
    let current = status(paths);
    if current.running {
        print_status(&current);
        return Ok(());
    }
    remove_stale_files(paths);

    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log)
        .context("failed to open daemon log")?;
    let mut child = Command::new(std::env::current_exe().context("missing current executable")?)
        .arg("__smd")
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            log.try_clone().context("failed to clone daemon log")?,
        ))
        .stderr(Stdio::from(log))
        .spawn()
        .context("failed to spawn daemon")?;

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let current = status(paths);
        if current.running {
            print_status(&current);
            return Ok(());
        }
        if let Some(exit) = child.try_wait().context("failed to observe daemon")? {
            bail!("daemon exited before becoming ready: {exit}");
        }
        thread::sleep(Duration::from_millis(100));
    }

    bail!("daemon did not become ready within 5s")
}

async fn stop(paths: &SmPaths) -> Result<()> {
    let current = status(paths);
    if !current.running {
        print_status(&current);
        return Ok(());
    }

    if paths.socket.exists() {
        let _ = sm_daemon::send_request(&paths.socket, &RpcRequest::Shutdown).await;
    }

    wait_for_stop(current.pid, Duration::from_secs(5));
    if let Some(pid) = current.pid
        && process_alive(pid)
    {
        let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
        wait_for_stop(Some(pid), Duration::from_millis(500));
    }
    if let Some(pid) = current.pid
        && process_alive(pid)
    {
        let _ = kill(Pid::from_raw(pid as i32), Signal::SIGKILL);
        wait_for_stop(Some(pid), Duration::from_millis(500));
    }
    if let Some(pid) = current.pid
        && process_alive(pid)
    {
        bail!("daemon process {pid} did not stop");
    }

    remove_stale_files(paths);
    print_status(&status(paths));
    Ok(())
}

fn wait_for_stop(pid: Option<u32>, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if pid.is_none_or(|pid| !process_alive(pid)) {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn status(paths: &SmPaths) -> DaemonStatus {
    let pid = read_pid(paths);
    DaemonStatus {
        running: pid.is_some_and(process_alive) && paths.socket.exists(),
        pid,
        pidfile: paths.pidfile.display().to_string(),
        socket: paths.socket.display().to_string(),
    }
}

fn read_pid(paths: &SmPaths) -> Option<u32> {
    fs::read_to_string(&paths.pidfile)
        .ok()
        .and_then(|pid| pid.trim().parse().ok())
}

fn process_alive(pid: u32) -> bool {
    kill(Pid::from_raw(pid as i32), None).is_ok()
}

fn remove_stale_files(paths: &SmPaths) {
    let _ = fs::remove_file(&paths.socket);
    let _ = fs::remove_file(&paths.pidfile);
}

fn print_status(status: &DaemonStatus) {
    println!("{}", if status.running { "running" } else { "stopped" });
    if let Some(pid) = status.pid {
        println!("pid: {pid}");
    }
    println!("pidfile: {}", status.pidfile);
    println!("socket: {}", status.socket);
}
