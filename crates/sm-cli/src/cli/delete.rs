use anyhow::{Result, bail};
use sm_core::{DeleteRequest, RpcRequest, RpcResponse, SmPaths};

use crate::cli::cli_def::{DeleteArgs, DeleteResource};
use crate::cli::output::print_session_line;

pub async fn run(args: DeleteArgs) -> Result<()> {
    match args.resource {
        DeleteResource::Agent => delete_agent(args).await,
    }
}

async fn delete_agent(args: DeleteArgs) -> Result<()> {
    let paths = SmPaths::from_env()?;
    let response = sm_daemon::send_request(
        &paths.socket,
        &RpcRequest::Delete {
            request: DeleteRequest {
                id: args.id,
                signal: args.signal,
                grace_secs: args.grace,
            },
        },
    )
    .await?;

    match response {
        RpcResponse::Deleted { response } => {
            print_session_line(&response.session);
            Ok(())
        }
        RpcResponse::Error { message } => bail!(message),
        other => bail!("unexpected daemon response: {other:?}"),
    }
}
