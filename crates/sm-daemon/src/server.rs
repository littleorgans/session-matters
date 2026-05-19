use std::fs;
use std::sync::Arc;

use anyhow::{Context, Result};
use sm_core::{RpcRequest, RpcResponse, SmEndpoint, SmPaths};
use sm_driver::InProcessDriver;
use sm_store::SqliteStore;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use crate::handler::DaemonState;
use crate::identity_client::{IdentityClient, RequestContext};
use crate::lifecycle::LifecycleTask;
use crate::reconcile::ReconcileTask;

pub async fn run_daemon(paths: SmPaths) -> Result<()> {
    fs::create_dir_all(&paths.dir).context("failed to create runtime directory")?;
    let endpoint = SmEndpoint::from_env().context("failed to resolve daemon endpoint")?;
    remove_stale_socket(&endpoint)?;

    let listener =
        UnixListener::bind(endpoint.as_path()).context("failed to bind daemon socket")?;
    fs::write(&paths.pidfile, std::process::id().to_string()).context("failed to write pidfile")?;

    let store = SqliteStore::open(&paths.database).context("failed to open sqlite store")?;
    let driver = InProcessDriver::new().context("failed to initialize in-process driver")?;
    let identity = IdentityClient::connect_default()
        .await
        .context("failed to initialize identity client")?;
    let state = Arc::new(DaemonState::new(
        store,
        Arc::new(driver),
        Arc::new(identity),
    ));
    crate::reconcile::reconcile_once(&state).context("failed to reconcile sessions on startup")?;
    let lifecycle = LifecycleTask::spawn(Arc::clone(&state));
    let reconcile = ReconcileTask::spawn(Arc::clone(&state));

    let result = serve(listener, &state).await;
    drop(reconcile);
    drop(lifecycle);
    state.driver.terminate_all();
    cleanup_paths(&paths, &endpoint);
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
    let principal = match lilo_im_core::peer_creds::extract(&stream).await {
        Ok(principal) => principal,
        Err(error) => {
            return write_response(
                stream,
                crate::handler::HandlerResult {
                    response: RpcResponse::Error {
                        message: error.to_string(),
                    },
                    shutdown: false,
                },
            )
            .await;
        }
    };

    let mut request_bytes = Vec::new();
    stream
        .read_to_end(&mut request_bytes)
        .await
        .context("failed to read request")?;

    let result = match serde_json::from_slice::<RpcRequest>(&request_bytes) {
        Ok(request) => state.handle(RequestContext::new(principal), request).await,
        Err(error) => crate::handler::HandlerResult {
            response: RpcResponse::Error {
                message: error.to_string(),
            },
            shutdown: false,
        },
    };

    write_response(stream, result).await
}

async fn write_response(
    mut stream: UnixStream,
    result: crate::handler::HandlerResult,
) -> Result<bool> {
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

fn remove_stale_socket(endpoint: &SmEndpoint) -> Result<()> {
    if endpoint.exists() {
        fs::remove_file(endpoint.as_path()).context("failed to remove stale socket")?;
    }
    Ok(())
}

fn cleanup_paths(paths: &SmPaths, endpoint: &SmEndpoint) {
    let _ = fs::remove_file(endpoint.as_path());
    let _ = fs::remove_file(&paths.pidfile);
}
