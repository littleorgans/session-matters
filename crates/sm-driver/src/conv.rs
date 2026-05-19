use lilo_rm_core::{Lifecycle, LifecycleState};

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
        });
    };

    if runtime_pid != expected_runtime_pid {
        return Ok(DriverProbe {
            verified: false,
            evidence: format!(
                "stored runtime pid {expected_runtime_pid} does not match rtmd pid {runtime_pid}"
            ),
        });
    }

    match lifecycle.state {
        LifecycleState::Forking | LifecycleState::Running => Ok(DriverProbe {
            verified: true,
            evidence: "rtmd lifecycle is active".to_string(),
        }),
        LifecycleState::Exited(exit) => Ok(DriverProbe {
            verified: false,
            evidence: format!("rtmd lifecycle exited: {exit}"),
        }),
        LifecycleState::Lost(evidence) => Ok(DriverProbe {
            verified: false,
            evidence: format!("rtmd lifecycle lost: {evidence}"),
        }),
        _ => Err(DriverError::UnknownRuntimeVariant {
            variant: format!("{:?}", lifecycle.state),
        }),
    }
}
