use anyhow::{Context, Result, bail};
use std::str::FromStr;

use lilo_rm_core::SpawnTarget;
use sm_core::{Label, Namespace, RpcRequest, RpcResponse, SmEndpoint, SpawnRequest};

use crate::cli::cli_def::RunArgs;
use crate::cli::namespace_resolver::resolve_namespace_dir;
use crate::cli::output::print_session_line;

pub async fn run(args: RunArgs) -> Result<()> {
    let spawn_location = resolve_spawn_location(&args)?;
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
                workspace: spawn_location.dir.clone(),
                dir: Some(spawn_location.dir),
                namespace: Some(spawn_location.namespace),
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct SpawnLocation {
    namespace: Namespace,
    dir: String,
}

fn resolve_spawn_location(args: &RunArgs) -> Result<SpawnLocation> {
    let start_dir = match &args.dir {
        Some(dir) => dir.clone(),
        None => {
            std::env::current_dir().context("cannot read current directory to resolve --dir")?
        }
    };
    let (namespace, canonical_dir) =
        resolve_namespace_dir(&start_dir, args.namespace.clone())?.into_pair();

    Ok(SpawnLocation {
        namespace,
        dir: canonical_dir.display().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::cli::cli_def::RunArgs;

    fn run_args(dir: Option<PathBuf>, namespace: Option<Namespace>) -> RunArgs {
        RunArgs {
            runtime: sm_core::RuntimeKind::Claude,
            role: "engineer".to_string(),
            dir,
            namespace,
            labels: Vec::new(),
            agent_config: None,
            target: "headless".to_string(),
            detach: true,
        }
    }

    #[test]
    fn explicit_namespace_uses_dir_as_canonical_spawn_dir() {
        let root = tempfile::tempdir().expect("tempdir");
        let project = root.path().join("project");
        std::fs::create_dir_all(&project).expect("project dir");
        let namespace = Namespace::new("alpha").expect("namespace");

        let resolved =
            resolve_spawn_location(&run_args(Some(project.clone()), Some(namespace.clone())))
                .expect("resolves");

        assert_eq!(resolved.namespace, namespace);
        assert_eq!(
            PathBuf::from(resolved.dir),
            std::fs::canonicalize(project).expect("canonical project")
        );
    }

    #[test]
    fn marker_walk_up_from_dir_sets_namespace_and_marker_dir() {
        let root = tempfile::tempdir().expect("tempdir");
        let project = root.path().join("project");
        let child = project.join("child");
        std::fs::create_dir_all(project.join(".sm")).expect("marker parent");
        std::fs::create_dir_all(&child).expect("child dir");
        std::fs::write(project.join(".sm").join("namespace"), "marker").expect("marker");

        let resolved =
            resolve_spawn_location(&run_args(Some(child), None)).expect("resolves marker");

        assert_eq!(resolved.namespace.as_str(), "marker");
        assert_eq!(
            PathBuf::from(resolved.dir),
            std::fs::canonicalize(project).expect("canonical project")
        );
    }

    #[test]
    fn marker_walk_up_from_cwd_when_dir_is_omitted() {
        let root = tempfile::tempdir().expect("tempdir");
        let project = root.path().join("project");
        let child = project.join("child");
        std::fs::create_dir_all(project.join(".sm")).expect("marker parent");
        std::fs::create_dir_all(&child).expect("child dir");
        std::fs::write(project.join(".sm").join("namespace"), "cwd-marker").expect("marker");
        let saved = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(&child).expect("chdir");

        let resolved = resolve_spawn_location(&run_args(None, None)).expect("resolves cwd marker");

        std::env::set_current_dir(saved).expect("restore cwd");
        assert_eq!(resolved.namespace.as_str(), "cwd-marker");
        assert_eq!(
            PathBuf::from(resolved.dir),
            std::fs::canonicalize(project).expect("canonical project")
        );
    }

    #[test]
    fn falls_back_to_default_namespace() {
        let project = tempfile::tempdir().expect("tempdir");

        let resolved = resolve_spawn_location(&run_args(Some(project.path().to_path_buf()), None))
            .expect("resolves default");

        assert_eq!(resolved.namespace, Namespace::default());
        assert_eq!(
            PathBuf::from(resolved.dir),
            std::fs::canonicalize(project.path()).expect("canonical project")
        );
    }
}
