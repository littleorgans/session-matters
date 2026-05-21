use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use std::str::FromStr;

use lilo_rm_core::SpawnTarget;
use sm_core::{Label, RpcRequest, RpcResponse, SmEndpoint, SpawnRequest};

use crate::cli::cli_def::RunArgs;
use crate::cli::output::print_session_line;

pub async fn run(args: RunArgs) -> Result<()> {
    let workspace = resolve_workspace(&args.workspace)?;
    let endpoint = SmEndpoint::from_env()?;
    let env = lilo_rm_core::capture_caller_env();
    let target = SpawnTarget::from_str(&args.target).ok();
    let shell_resume = if target
        .as_ref()
        .and_then(SpawnTarget::tmux_address)
        .is_some()
    {
        Some(lilo_rm_core::capture_shell_resume(
            lilo_rm_core::capture_caller_cwd()?,
        ))
    } else {
        None
    };
    let response = sm_daemon::send_request(
        &endpoint,
        &RpcRequest::Spawn {
            request: SpawnRequest {
                runtime: args.runtime,
                role: args.role,
                workspace,
                dir: None,
                namespace: None,
                target: args.target,
                agent_config: args.agent_config,
                env,
                shell_resume,
                labels: args
                    .labels
                    .iter()
                    .map(|label| Label::from_str(label))
                    .collect::<Result<Vec<_>, _>>()?,
            },
        },
    )
    .await?;

    match response {
        RpcResponse::Spawned { response } => {
            print_session_line(&response.session);
            Ok(())
        }
        RpcResponse::Error { message } => bail!(message),
        other => bail!(
            "unexpected daemon response: {} (please report)",
            other.kind()
        ),
    }
}

fn resolve_workspace(raw: &str) -> Result<String> {
    if raw.is_empty() {
        bail!("--workspace must not be empty");
    }
    let path = PathBuf::from(raw);
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .context("cannot read current directory to resolve --workspace")?
            .join(&path)
    };
    let canonical = std::fs::canonicalize(&absolute).with_context(|| {
        format!(
            "--workspace must point to an existing directory: {}",
            absolute.display()
        )
    })?;
    if !canonical.is_dir() {
        bail!("--workspace must be a directory: {}", canonical.display());
    }
    Ok(canonical.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_relative_path_against_cwd() {
        let dir = tempfile::tempdir().expect("tempdir");
        let saved = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(dir.path()).expect("chdir");
        let resolved = resolve_workspace(".").expect("resolves");
        std::env::set_current_dir(saved).expect("restore cwd");
        assert_eq!(
            std::fs::canonicalize(&resolved).expect("canonical"),
            std::fs::canonicalize(dir.path()).expect("canonical tempdir")
        );
    }

    #[test]
    fn rejects_missing_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("does-not-exist");
        let err = resolve_workspace(missing.to_str().expect("utf8")).unwrap_err();
        assert!(err.to_string().contains("existing directory"));
    }
}
