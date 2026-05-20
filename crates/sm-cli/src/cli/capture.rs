use anyhow::{Result, bail};
use lilo_rm_core::{CaptureError, CaptureResponse};
use std::str::FromStr;

use sm_core::{CaptureRequest, RpcRequest, RpcResponse, Selector, SmEndpoint};

use crate::cli::cli_def::CaptureArgs;

pub async fn run(args: CaptureArgs) -> Result<()> {
    let endpoint = SmEndpoint::from_env()?;
    let response = sm_daemon::send_request(
        &endpoint,
        &RpcRequest::Capture {
            request: CaptureRequest {
                selector: Selector::from_str(&args.selector)?,
                scrollback_lines: args.scrollback_lines,
            },
        },
    )
    .await?;

    match response {
        RpcResponse::Capture { response } if args.json => {
            println!("{}", serde_json::to_string_pretty(&response)?);
            Ok(())
        }
        RpcResponse::Capture { response } => print_capture(response.capture),
        RpcResponse::Error { message } => bail!(message),
        other => bail!("unexpected daemon response: {other:?}"),
    }
}

fn print_capture(response: CaptureResponse) -> Result<()> {
    match response {
        CaptureResponse::Captured(snapshot) => {
            print!("{}", snapshot.content);
            Ok(())
        }
        CaptureResponse::Failed(error) => {
            eprintln!("{}", capture_error_message(&error));
            std::process::exit(capture_exit_code(&error));
        }
        _ => bail!("unsupported capture response"),
    }
}

fn capture_error_message(error: &CaptureError) -> String {
    match error {
        CaptureError::NotATmuxTarget => "session is not a tmux target".to_string(),
        CaptureError::PaneUnavailable => "tmux pane is unavailable".to_string(),
        CaptureError::SessionMissing => "runtime session is missing".to_string(),
        CaptureError::TmuxNotAvailable => "tmux is not available".to_string(),
        CaptureError::CapturePaneFailed { stderr } => {
            format!("tmux capture-pane failed: {stderr}")
        }
        _ => "unsupported capture failure".to_string(),
    }
}

fn capture_exit_code(error: &CaptureError) -> i32 {
    match error {
        CaptureError::NotATmuxTarget => 2,
        CaptureError::PaneUnavailable | CaptureError::SessionMissing => 3,
        CaptureError::TmuxNotAvailable | CaptureError::CapturePaneFailed { .. } => 4,
        _ => 4,
    }
}
