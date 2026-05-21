use anyhow::{Result, bail};

use sm_core::{NudgeRequest, RpcRequest, RpcResponse, SmEndpoint};

use crate::cli::cli_def::NudgeArgs;
use crate::cli::selector_scope::scoped_selector;

pub async fn run(args: NudgeArgs) -> Result<()> {
    let endpoint = SmEndpoint::from_env()?;
    let response = sm_daemon::send_request(
        &endpoint,
        &RpcRequest::Nudge {
            request: NudgeRequest {
                to: scoped_selector(Some(&args.to), &args.scope)?.expect("selector is present"),
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
