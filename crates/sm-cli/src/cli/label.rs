use std::str::FromStr;

use anyhow::{Result, bail};
use sm_core::{LabelMutation, LabelRequest, RpcRequest, RpcResponse, SmEndpoint};

use crate::cli::cli_def::LabelArgs;
use crate::cli::output::print_session_line;
use crate::cli::selector_scope::scoped_selector;

pub async fn run(args: LabelArgs) -> Result<()> {
    let endpoint = SmEndpoint::from_env()?;
    let response = sm_daemon::send_request(
        &endpoint,
        &RpcRequest::Label {
            request: LabelRequest {
                selector: scoped_selector(Some(&args.selector), &args.scope)?
                    .expect("selector is present"),
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
        other => bail!(
            "unexpected daemon response: {} (please report)",
            other.kind()
        ),
    }
}
