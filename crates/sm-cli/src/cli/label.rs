use std::str::FromStr;

use anyhow::{Result, bail};
use sm_core::{LabelMutation, LabelRequest, RpcRequest, RpcResponse, Selector, SmPaths};

use crate::cli::cli_def::LabelArgs;
use crate::cli::output::print_session_line;

pub async fn run(args: LabelArgs) -> Result<()> {
    let paths = SmPaths::from_env()?;
    let response = sm_daemon::send_request(
        &paths.socket,
        &RpcRequest::Label {
            request: LabelRequest {
                selector: Selector::from_str(&args.selector)?,
                mutation: LabelMutation::from_str(&args.mutation)?,
            },
        },
    )
    .await?;

    match response {
        RpcResponse::Labeled { response } => {
            for session in response.sessions {
                print_session_line(&session);
            }
            for error in response.errors {
                eprintln!("{} {}", error.target, error.message);
            }
            Ok(())
        }
        RpcResponse::Error { message } => bail!(message),
        other => bail!("unexpected daemon response: {other:?}"),
    }
}
