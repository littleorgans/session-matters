use std::str::FromStr;

use anyhow::{Context, Result, bail};
use sm_core::{LinkRequest, RpcRequest, RpcResponse, Selector, SmEndpoint};
use uuid::Uuid;

use crate::cli::cli_def::LinkArgs;
use crate::cli::output::print_session_line;

pub async fn run(args: LinkArgs) -> Result<()> {
    let endpoint = SmEndpoint::from_env()?;
    let response = sm_daemon::send_request(
        &endpoint,
        &RpcRequest::Link {
            request: LinkRequest {
                session_id: link_session_id(args.session_id.as_deref())?,
                selector: args
                    .selector
                    .as_deref()
                    .map(Selector::from_str)
                    .transpose()?,
                runtime_session: args.runtime_session,
                transcript_path: args.transcript,
            },
        },
    )
    .await?;

    match response {
        RpcResponse::Linked { response } => {
            print_session_line(&response.session, false);
            Ok(())
        }
        RpcResponse::Error { message } => bail!(message),
        other => bail!(
            "unexpected daemon response: {} (please report)",
            other.kind()
        ),
    }
}

fn link_session_id(explicit: Option<&str>) -> Result<Option<Uuid>> {
    explicit
        .map(ToString::to_string)
        .or_else(|| std::env::var("HELIOY_SESSION_ID").ok())
        .as_deref()
        .map(|id| Uuid::parse_str(id).context("invalid link session id"))
        .transpose()
}
