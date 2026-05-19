use anyhow::{Result, bail};
use std::str::FromStr;

use sm_core::{ListRequest, RpcRequest, RpcResponse, Selector, SmEndpoint};

use crate::cli::cli_def::{GetArgs, GetResource};
use crate::cli::output::{print_session_line, print_session_table};

pub async fn run(args: GetArgs) -> Result<()> {
    match args.resource {
        GetResource::Agent => get_agent(args).await,
        GetResource::Agents => list_agents(args).await,
    }
}

async fn get_agent(args: GetArgs) -> Result<()> {
    let id = args
        .id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("sm get agent requires a session id"))?;
    let response = send_list(Some(Selector::from_str(id)?)).await?;

    match response {
        RpcResponse::Listed { response } if args.json => {
            let session = response
                .sessions
                .into_iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("unknown session: {id}"))?;
            println!("{}", serde_json::to_string_pretty(&session)?);
            Ok(())
        }
        RpcResponse::Listed { response } => {
            let session = response
                .sessions
                .first()
                .ok_or_else(|| anyhow::anyhow!("unknown session: {id}"))?;
            print_session_line(session);
            Ok(())
        }
        RpcResponse::Error { message } => bail!(message),
        other => bail!("unexpected daemon response: {other:?}"),
    }
}

async fn list_agents(args: GetArgs) -> Result<()> {
    let response = send_list(
        args.selector
            .as_deref()
            .map(Selector::from_str)
            .transpose()?,
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

async fn send_list(selector: Option<Selector>) -> Result<RpcResponse> {
    let endpoint = SmEndpoint::from_env()?;
    sm_daemon::send_request(
        &endpoint,
        &RpcRequest::List {
            request: ListRequest { selector },
        },
    )
    .await
}
