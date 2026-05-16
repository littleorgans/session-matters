use anyhow::{Result, bail};
use sm_core::{RpcRequest, RpcResponse, SmPaths, SpawnRequest};

use crate::cli::cli_def::RunArgs;
use crate::cli::output::print_session_line;

pub async fn run(args: RunArgs) -> Result<()> {
    if !args.detach {
        eprintln!("attached mode is deferred in pass 1; leaving session detached");
    }

    let paths = SmPaths::from_env()?;
    let response = sm_daemon::send_request(
        &paths.socket,
        &RpcRequest::Spawn {
            request: SpawnRequest {
                runtime: args.runtime,
                role: args.role,
                workspace: args.workspace,
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
