use anyhow::{Result, bail};

use sm_core::{ListRequest, RpcRequest, RpcResponse, Selector, SmEndpoint};

use crate::cli::cli_def::{GetArgs, GetResource};
use crate::cli::output::{print_session_line, print_session_table};
use crate::cli::selector_scope::scoped_selector;

pub async fn run(args: GetArgs) -> Result<()> {
    match args.resource {
        GetResource::Agent if args.id.is_some() => get_agent(args).await,
        GetResource::Agent => list_agents(args).await,
        GetResource::Agents => list_agents(args).await,
        GetResource::Namespace => crate::cli::namespace::get(args.id, args.json).await,
    }
}

async fn get_agent(args: GetArgs) -> Result<()> {
    let id = args
        .id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("sm get agent requires a session id"))?;
    let response = send_list(scoped_selector(Some(id), &args.scope)?).await?;

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
        other => bail!(
            "unexpected daemon response: {} (please report)",
            other.kind()
        ),
    }
}

async fn list_agents(args: GetArgs) -> Result<()> {
    let response = send_list(scoped_selector(args.selector.as_deref(), &args.scope)?).await?;

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
        other => bail!(
            "unexpected daemon response: {} (please report)",
            other.kind()
        ),
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
