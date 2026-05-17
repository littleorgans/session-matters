use anyhow::{Result, bail};
use std::str::FromStr;

use sm_core::{ListRequest, RpcRequest, RpcResponse, Selector, SmPaths};

use crate::cli::cli_def::{GetArgs, GetResource};
use crate::cli::output::print_session_table;

pub async fn run(args: GetArgs) -> Result<()> {
    match args.resource {
        GetResource::Agents => list_agents(args).await,
    }
}

async fn list_agents(args: GetArgs) -> Result<()> {
    let paths = SmPaths::from_env()?;
    let response = sm_daemon::send_request(
        &paths.socket,
        &RpcRequest::List {
            request: ListRequest {
                selector: args
                    .selector
                    .as_deref()
                    .map(Selector::from_str)
                    .transpose()?,
            },
        },
    )
    .await?;

    match response {
        RpcResponse::Listed { response } if args.json => {
            println!("{}", serde_json::to_string_pretty(&response.sessions)?);
            Ok(())
        }
        RpcResponse::Listed { response } => {
            print_session_table(&response.sessions);
            Ok(())
        }
        RpcResponse::Error { message } => bail!(message),
        other => bail!("unexpected daemon response: {other:?}"),
    }
}
