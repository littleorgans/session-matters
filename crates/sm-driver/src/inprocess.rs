use std::collections::HashMap;
use std::ffi::CString;
use std::os::fd::OwnedFd;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use nix::pty::{ForkptyResult, forkpty};
use nix::sys::signal::{Signal, kill};
use nix::unistd::{Pid, execvp};
use sm_core::SpawnRequest;

use crate::driver::{DriverError, SpawnDriver, SpawnedProcess};

#[derive(Default)]
pub struct InProcessDriver {
    children: Mutex<HashMap<String, SpawnHandle>>,
}

struct SpawnHandle {
    pid: Pid,
    _master: OwnedFd,
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

        thread::sleep(Duration::from_millis(200));

        for handle in handles {
            if kill(handle.pid, None).is_ok() {
                let _ = kill(handle.pid, Signal::SIGKILL);
            }
        }
    }
}

fn runtime_pid(pid: Pid) -> Result<u32, DriverError> {
    u32::try_from(pid.as_raw()).map_err(|_| DriverError::PidOutOfRange(pid.as_raw()))
}
