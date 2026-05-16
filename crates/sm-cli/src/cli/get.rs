use anyhow::{Result, bail};
use sm_core::{ListRequest, RpcRequest, RpcResponse, Session, SmPaths};

use crate::cli::cli_def::{GetArgs, GetResource};

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
            request: ListRequest { id: args.id },
        },
    )
    .await?;

    match response {
        RpcResponse::Listed { response } if args.json => {
            println!("{}", serde_json::to_string_pretty(&response.sessions)?);
            Ok(())
        }
        RpcResponse::Listed { response } => {
            print_sessions(&response.sessions);
            Ok(())
        }
        RpcResponse::Error { message } => bail!(message),
        other => bail!("unexpected daemon response: {other:?}"),
    }
}

fn print_sessions(sessions: &[Session]) {
    println!("ID RUNTIME ROLE WORKSPACE STATE PID");
    for session in sessions {
        println!(
            "{} {} {} {} {} {}",
            session.id,
            session.runtime,
            session.role,
            session.workspace,
            session.state,
            session.runtime_pid
        );
    }
}
