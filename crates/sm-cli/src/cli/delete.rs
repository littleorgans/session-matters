use anyhow::{Result, bail};

use sm_core::{DeleteRequest, RpcRequest, RpcResponse, SmEndpoint};

use crate::cli::cli_def::{DeleteArgs, DeleteResource};
use crate::cli::output::print_session_line;
use crate::cli::selector_scope::scoped_selector;

pub async fn run(args: DeleteArgs) -> Result<()> {
    match args.resource {
        DeleteResource::Agent => delete_agent(args).await,
    }
}

async fn delete_agent(args: DeleteArgs) -> Result<()> {
    let endpoint = SmEndpoint::from_env()?;
    let response = sm_daemon::send_request(
        &endpoint,
        &RpcRequest::Delete {
            request: DeleteRequest {
                selector: scoped_selector(Some(&args.selector), &args.scope)?
                    .expect("selector is present"),
                signal: args.signal,
                grace_secs: args.grace,
            },
        },
    )
    .await?;

    match response {
        RpcResponse::Deleted { response } => {
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
