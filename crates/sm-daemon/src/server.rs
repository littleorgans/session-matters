use std::fs;
use std::sync::Arc;

use anyhow::{Context, Result};
use sm_core::{RpcRequest, RpcResponse, SmPaths};
use sm_driver::InProcessDriver;
use sm_store::SqliteStore;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use crate::handler::DaemonState;

pub async fn run_daemon(paths: SmPaths) -> Result<()> {
    fs::create_dir_all(&paths.dir).context("failed to create runtime directory")?;
    remove_stale_socket(&paths)?;

    let listener = UnixListener::bind(&paths.socket).context("failed to bind daemon socket")?;
    fs::write(&paths.pidfile, std::process::id().to_string()).context("failed to write pidfile")?;

    let store = SqliteStore::open(&paths.database).context("failed to open sqlite store")?;
    let state = DaemonState::new(store, Arc::new(InProcessDriver::default()));

    let result = serve(listener, &state).await;
    state.driver.terminate_all();
    cleanup_paths(&paths);
    result
}

async fn serve(listener: UnixListener, state: &DaemonState) -> Result<()> {
    loop {
        let (stream, _) = listener.accept().await.context("failed to accept client")?;
        if handle_connection(stream, state).await? {
            return Ok(());
        }
    }
}

async fn handle_connection(mut stream: UnixStream, state: &DaemonState) -> Result<bool> {
    let mut request_bytes = Vec::new();
    stream
        .read_to_end(&mut request_bytes)
        .await
        .context("failed to read request")?;

    let result = match serde_json::from_slice::<RpcRequest>(&request_bytes) {
        Ok(request) => state.handle(request),
        Err(error) => crate::handler::HandlerResult {
            response: RpcResponse::Error {
                message: error.to_string(),
            },
            shutdown: false,
        },
    };

    let response = serde_json::to_vec(&result.response).context("failed to encode response")?;
    stream
        .write_all(&response)
        .await
        .context("failed to write response")?;
    stream
        .shutdown()
        .await
        .context("failed to close response")?;

    Ok(result.shutdown)
}

fn remove_stale_socket(paths: &SmPaths) -> Result<()> {
    if paths.socket.exists() {
        fs::remove_file(&paths.socket).context("failed to remove stale socket")?;
    }
    Ok(())
}

fn cleanup_paths(paths: &SmPaths) {
    let _ = fs::remove_file(&paths.socket);
    let _ = fs::remove_file(&paths.pidfile);
}
