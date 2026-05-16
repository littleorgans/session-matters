use std::collections::HashMap;
use std::ffi::CString;
use std::os::fd::OwnedFd;
use std::os::raw::c_int;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once};
use std::thread;
use std::time::{Duration, Instant};

use nix::pty::{ForkptyResult, forkpty};
use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, kill, sigaction};
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::{Pid, execvp};
use sm_core::SpawnRequest;

use crate::driver::{ChildExit, DriverError, NudgeResult, SpawnDriver, SpawnedProcess};

pub struct InProcessDriver {
    children: Mutex<HashMap<String, SpawnHandle>>,
}

struct SpawnHandle {
    pid: Pid,
    _master: OwnedFd,
}

static SIGCHLD_SEEN: AtomicBool = AtomicBool::new(false);
static INSTALL_SIGCHLD: Once = Once::new();

impl InProcessDriver {
    pub fn new() -> Result<Self, DriverError> {
        install_sigchld_handler();
        Ok(Self {
            children: Mutex::new(HashMap::new()),
        })
    }
}

impl SpawnDriver for InProcessDriver {
    fn spawn(
        &self,
        session_id: &str,
        request: &SpawnRequest,
    ) -> Result<SpawnedProcess, DriverError> {
        let command = CString::new(request.runtime.command())
            .map_err(|_| DriverError::InvalidRuntimeCommand)?;

        match unsafe { forkpty(None, None)? } {
            ForkptyResult::Parent { child, master } => {
                let runtime_pid = runtime_pid(child)?;
                let handle = SpawnHandle {
                    pid: child,
                    _master: master,
                };
                self.children
                    .lock()
                    .expect("driver child registry poisoned")
                    .insert(session_id.to_string(), handle);
                Ok(SpawnedProcess { runtime_pid })
            }
            ForkptyResult::Child => {
                let args = [command.as_c_str()];
                let _ = execvp(&command, &args);
                std::process::exit(127);
            }
        }
    }

    fn reap_exited(&self) -> Result<Vec<ChildExit>, DriverError> {
        let _ = SIGCHLD_SEEN.swap(false, Ordering::SeqCst);
        let mut children = self
            .children
            .lock()
            .expect("driver child registry poisoned");
        let child_refs = children
            .iter()
            .map(|(session_id, handle)| (session_id.clone(), handle.pid))
            .collect::<Vec<_>>();
        let mut exits = Vec::new();

        for (session_id, pid) in child_refs {
            if let Some(exit) = observe_exit(&session_id, pid)? {
                children.remove(&session_id);
                exits.push(exit);
            }
        }

        Ok(exits)
    }

    fn terminate(
        &self,
        session_id: &str,
        signal: &str,
        grace: Duration,
    ) -> Result<Option<ChildExit>, DriverError> {
        let handle = self
            .children
            .lock()
            .expect("driver child registry poisoned")
            .remove(session_id);
        let Some(handle) = handle else {
            return Ok(None);
        };

        let pid = handle.pid;
        let signal = parse_signal(signal)?;
        let _ = kill(pid, signal);
        drop(handle);
        if let Some(exit) = wait_for_exit(session_id, pid, grace)? {
            return Ok(Some(exit));
        }

        let _ = kill(pid, Signal::SIGKILL);
        wait_for_exit(session_id, pid, Duration::from_secs(1))?
            .ok_or(DriverError::TerminationTimeout)
            .map(Some)
    }

    fn nudge(&self, _session_id: &str, _content: &str) -> Result<NudgeResult, DriverError> {
        eprintln!("nudge: tmux gateway not available; nudge skipped");
        Ok(NudgeResult {
            delivered: false,
            message: "nudge: tmux gateway not available; nudge skipped".to_string(),
        })
    }

    fn terminate_all(&self) {
        let handles = self
            .children
            .lock()
            .expect("driver child registry poisoned")
            .drain()
            .map(|(_, handle)| handle)
            .collect::<Vec<_>>();

        for handle in &handles {
            let _ = kill(handle.pid, Signal::SIGTERM);
        }
        let pids = handles
            .into_iter()
            .map(|handle| handle.pid)
            .collect::<Vec<_>>();

        thread::sleep(Duration::from_millis(200));

        for pid in pids {
            if kill(pid, None).is_ok() {
                let _ = kill(pid, Signal::SIGKILL);
            }
        }
    }
}

fn runtime_pid(pid: Pid) -> Result<u32, DriverError> {
    u32::try_from(pid.as_raw()).map_err(|_| DriverError::PidOutOfRange(pid.as_raw()))
}

fn install_sigchld_handler() {
    INSTALL_SIGCHLD.call_once(|| {
        let action = SigAction::new(
            SigHandler::Handler(mark_sigchld),
            SaFlags::SA_RESTART,
            SigSet::empty(),
        );
        let _ = unsafe { sigaction(Signal::SIGCHLD, &action) };
    });
}

extern "C" fn mark_sigchld(_: c_int) {
    SIGCHLD_SEEN.store(true, Ordering::SeqCst);
}

fn wait_for_exit(
    session_id: &str,
    pid: Pid,
    timeout: Duration,
) -> Result<Option<ChildExit>, DriverError> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(exit) = observe_exit(session_id, pid)? {
            return Ok(Some(exit));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn observe_exit(session_id: &str, pid: Pid) -> Result<Option<ChildExit>, DriverError> {
    match waitpid(pid, Some(WaitPidFlag::WNOHANG))? {
        WaitStatus::StillAlive => Ok(None),
        WaitStatus::Exited(_, code) => child_exit(session_id, pid, Some(code)).map(Some),
        WaitStatus::Signaled(_, signal, _) => {
            child_exit(session_id, pid, Some(128 + signal as i32)).map(Some)
        }
        _ => Ok(None),
    }
}

fn child_exit(
    session_id: &str,
    pid: Pid,
    exit_code: Option<i32>,
) -> Result<ChildExit, DriverError> {
    Ok(ChildExit {
        session_id: session_id.to_string(),
        runtime_pid: runtime_pid(pid)?,
        exit_code,
    })
}

fn parse_signal(signal: &str) -> Result<Signal, DriverError> {
    let normalized = signal.trim().trim_start_matches("SIG").to_ascii_uppercase();
    if let Ok(number) = normalized.parse::<i32>() {
        return Signal::try_from(number)
            .map_err(|_| DriverError::InvalidSignal(signal.to_string()));
    }

    match normalized.as_str() {
        "TERM" => Ok(Signal::SIGTERM),
        "KILL" => Ok(Signal::SIGKILL),
        "INT" => Ok(Signal::SIGINT),
        "HUP" => Ok(Signal::SIGHUP),
        "QUIT" => Ok(Signal::SIGQUIT),
        "ABRT" => Ok(Signal::SIGABRT),
        _ => Err(DriverError::InvalidSignal(signal.to_string())),
    }
}
