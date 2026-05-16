use anyhow::{Result, bail};
use sm_core::{NudgeRequest, RpcRequest, RpcResponse, SmPaths};

use crate::cli::cli_def::NudgeArgs;

pub async fn run(args: NudgeArgs) -> Result<()> {
    let paths = SmPaths::from_env()?;
    let response = sm_daemon::send_request(
        &paths.socket,
        &RpcRequest::Nudge {
            request: NudgeRequest {
                to: args.to,
                content: args.content,
            },
        },
    )
    .await?;

    match response {
        RpcResponse::Nudged { response } => {
            println!("{}", response.message);
            Ok(())
        }
        RpcResponse::Error { message } => bail!(message),
        other => bail!("unexpected daemon response: {other:?}"),
    }
}
