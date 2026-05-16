use anyhow::{Result, bail};
use sm_core::{McpBridgeRequest, RpcRequest, RpcResponse, SmPaths};
use tokio::io::{self, AsyncBufReadExt, BufReader};

use crate::mcp::transport::write_line;

pub async fn run_stdio_bridge(paths: SmPaths) -> Result<()> {
    let stdin = BufReader::new(io::stdin());
    let mut lines = stdin.lines();
    let mut stdout = io::stdout();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let response = sm_daemon::send_request(
            &paths.socket,
            &RpcRequest::McpBridge {
                request: McpBridgeRequest { line },
            },
        )
        .await?;

        match response {
            RpcResponse::McpBridge { response } => {
                if let Some(line) = response.line {
                    write_line(&mut stdout, &line).await?;
                }
            }
            RpcResponse::Error { message } => bail!(message),
            other => bail!("unexpected daemon response: {other:?}"),
        }
    }

    Ok(())
}
