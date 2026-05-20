use std::path::PathBuf;

use lilo_rm_core::{Lifecycle, LifecycleState, LogAvailability};

use crate::driver::{DriverError, DriverProbe};

pub(crate) fn lifecycle_to_probe(
    lifecycle: &Lifecycle,
    expected_runtime_pid: u32,
) -> Result<DriverProbe, DriverError> {
    let Some(runtime_pid) = lifecycle.runtime_pid else {
        return Ok(DriverProbe {
            verified: false,
            evidence: format!(
                "runtime session {} has no runtime pid",
                lifecycle.session_id
            ),
            transcript_path: lifecycle_transcript_path(lifecycle),
        });
    };

    if runtime_pid != expected_runtime_pid {
        return Ok(DriverProbe {
            verified: false,
            evidence: format!(
                "stored runtime pid {expected_runtime_pid} does not match rtmd pid {runtime_pid}"
            ),
            transcript_path: lifecycle_transcript_path(lifecycle),
        });
    }

    match lifecycle.state {
        LifecycleState::Forking | LifecycleState::Running => Ok(DriverProbe {
            verified: true,
            evidence: "rtmd lifecycle is active".to_string(),
            transcript_path: lifecycle_transcript_path(lifecycle),
        }),
        LifecycleState::Exited(exit) => Ok(DriverProbe {
            verified: false,
            evidence: format!("rtmd lifecycle exited: {exit}"),
            transcript_path: lifecycle_transcript_path(lifecycle),
        }),
        LifecycleState::Lost(evidence) => Ok(DriverProbe {
            verified: false,
            evidence: format!("rtmd lifecycle lost: {evidence}"),
            transcript_path: lifecycle_transcript_path(lifecycle),
        }),
        _ => Err(DriverError::UnknownRuntimeVariant {
            variant: format!("{:?}", lifecycle.state),
        }),
    }
}

pub(crate) fn lifecycle_transcript_path(lifecycle: &Lifecycle) -> Option<PathBuf> {
    match lifecycle.log_availability.as_ref() {
        Some(LogAvailability::Headless { stdout_path, .. }) => Some(stdout_path.clone()),
        Some(LogAvailability::TmuxPaneSnapshot | LogAvailability::Unavailable { .. }) | None => {
            None
        }
    }
}
