use anyhow::{Result, bail};
use std::str::FromStr;

use sm_core::{Label, RpcRequest, RpcResponse, SmEndpoint, SpawnRequest};

use crate::cli::cli_def::RunArgs;
use crate::cli::output::print_session_line;

pub async fn run(args: RunArgs) -> Result<()> {
    if !args.detach {
        eprintln!("attached mode is deferred in pass 1; leaving session detached");
    }

    let endpoint = SmEndpoint::from_env()?;
    let response = sm_daemon::send_request(
        &endpoint,
        &RpcRequest::Spawn {
            request: SpawnRequest {
                runtime: args.runtime,
                role: args.role,
                workspace: args.workspace,
                target: args.target,
                agent_config: args.agent_config,
                labels: args
                    .labels
                    .iter()
                    .map(|label| Label::from_str(label))
                    .collect::<Result<Vec<_>, _>>()?,
            },
        },
    )
    .await?;

    match response {
        RpcResponse::Spawned { response } => {
            print_session_line(&response.session);
            Ok(())
        }
        RpcResponse::Error { message } => bail!(message),
        other => bail!("unexpected daemon response: {other:?}"),
    }
}
