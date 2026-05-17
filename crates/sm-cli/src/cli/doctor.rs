use anyhow::{Result, bail};
use sm_core::{DoctorRequest, RpcRequest, RpcResponse, SmPaths};

use crate::cli::cli_def::DoctorArgs;

pub async fn run(_args: DoctorArgs) -> Result<()> {
    let paths = SmPaths::from_env()?;
    let response = sm_daemon::send_request(
        &paths.socket,
        &RpcRequest::Doctor {
            request: DoctorRequest::default(),
        },
    )
    .await?;

    match response {
        RpcResponse::Doctor { response } => {
            println!("status: {}", response.status);
            println!("runtime: {}", response.runtime);
            for finding in response.findings {
                println!(
                    "{} {} {}",
                    finding.severity,
                    finding.session_id.unwrap_or_else(|| "-".to_string()),
                    finding.message
                );
            }
            Ok(())
        }
        RpcResponse::Error { message } => bail!(message),
        other => bail!("unexpected daemon response: {other:?}"),
    }
}
