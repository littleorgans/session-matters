use std::collections::HashMap;
use std::ffi::CString;
use std::os::fd::OwnedFd;
use std::os::raw::c_int;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once};
use std::thread;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use nix::pty::{ForkptyResult, forkpty};
use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, kill, sigaction};
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::{Pid, execvp};

use crate::driver::{
    ChildExit, DriverError, DriverProbe, NudgeResult, SpawnDriver, SpawnLaunch, SpawnedProcess,
};

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

#[async_trait]
impl SpawnDriver for InProcessDriver {
    async fn spawn(
        &self,
        session_id: &str,
        launch: &SpawnLaunch,
    ) -> Result<SpawnedProcess, DriverError> {
        validate_env(launch)?;
        let command = CString::new(launch.runtime.command())
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
                Ok(SpawnedProcess {
                    runtime_pid,
                    log_dir: None,
                    stdout_path: None,
                    stderr_path: None,
                })
            }
            ForkptyResult::Child => {
                for item in &launch.env {
                    unsafe {
                        std::env::set_var(&item.key, &item.value);
                    }
                }
                let args = [command.as_c_str()];
                let _ = execvp(&command, &args);
                std::process::exit(127);
            }
        }
    }

    async fn reap_exited(&self) -> Result<Vec<ChildExit>, DriverError> {
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

    async fn probe_session(
        &self,
        session_id: &str,
        stored_pid: u32,
    ) -> Result<DriverProbe, DriverError> {
        let children = self
            .children
            .lock()
            .expect("driver child registry poisoned");
        let Some(handle) = children.get(session_id) else {
            return Ok(DriverProbe {
                verified: false,
                evidence: "session is not owned by this daemon".to_string(),
                transcript_path: None,
            });
        };
        if runtime_pid(handle.pid)? != stored_pid {
            return Ok(DriverProbe {
                verified: false,
                evidence: format!(
                    "stored runtime pid {stored_pid} does not match driver pid {}",
                    handle.pid
                ),
                transcript_path: None,
            });
        }
        let raw_pid =
            i32::try_from(stored_pid).map_err(|_| DriverError::StoredPidOutOfRange(stored_pid))?;
        let verified = kill(Pid::from_raw(raw_pid), None).is_ok();
        Ok(DriverProbe {
            verified,
            evidence: if verified {
                "runtime process is alive and owned by this daemon".to_string()
            } else {
                "runtime process is not alive".to_string()
            },
            transcript_path: None,
        })
    }

    async fn terminate(
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

    async fn nudge(&self, _session_id: &str, _content: &str) -> Result<NudgeResult, DriverError> {
        Err(DriverError::Unsupported {
            operation: "nudge",
            pass: "Pass 5",
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

fn validate_env(launch: &SpawnLaunch) -> Result<(), DriverError> {
    if launch
        .env
        .iter()
        .any(|item| item.key.contains('\0') || item.value.contains('\0'))
    {
        return Err(DriverError::InvalidEnvironment);
    }
    Ok(())
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
        transcript_path: None,
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
