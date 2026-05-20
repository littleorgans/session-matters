use anyhow::{Result, bail};
use std::str::FromStr;

use sm_core::{NudgeRequest, RpcRequest, RpcResponse, Selector, SmEndpoint};

use crate::cli::cli_def::NudgeArgs;

pub async fn run(args: NudgeArgs) -> Result<()> {
    let endpoint = SmEndpoint::from_env()?;
    let response = sm_daemon::send_request(
        &endpoint,
        &RpcRequest::Nudge {
            request: NudgeRequest {
                to: Selector::from_str(&args.to)?,
                content: args.content,
            },
        },
    )
    .await?;

    match response {
        RpcResponse::Nudged { response } => {
            for nudge in response.nudges {
                println!("{} {}", nudge.to, nudge.message);
            }
            for error in response.errors {
                eprintln!("{} {}", error.target, error.message);
            }
            Ok(())
        }
        RpcResponse::Error { message } => bail!(message),
        other => bail!(
            "unexpected daemon response: {} (please report)",
            other.kind()
        ),
    }
}
