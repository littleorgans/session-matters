use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use std::str::FromStr;

use lilo_rm_core::SpawnTarget;
use sm_core::{Label, Namespace, RpcRequest, RpcResponse, SmEndpoint, SpawnRequest};

use crate::cli::cli_def::{RunArgs, SessionCreateArgs};
use crate::cli::namespace_resolver::resolve_namespace_dir;
use crate::cli::output::print_session_line;

pub async fn run(args: RunArgs) -> Result<()> {
    spawn_session(SessionSpawnArgs {
        runtime: args.session.runtime,
        role: args.session.role,
        dir: args.session.dir,
        namespace: args.session.namespace,
        labels: args.session.labels,
        agent_config: args.session.agent_config,
        target: args.target,
    })
    .await
}

pub async fn create_session(args: SessionCreateArgs) -> Result<()> {
    spawn_session(SessionSpawnArgs {
        runtime: args.runtime,
        role: args.role,
        dir: args.dir,
        namespace: args.namespace,
        labels: args.labels,
        agent_config: args.agent_config,
        target: "headless".to_string(),
    })
    .await
}

async fn spawn_session(args: SessionSpawnArgs) -> Result<()> {
    let spawn_location = resolve_spawn_location(args.dir.as_ref(), args.namespace.clone())?;
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

#[derive(Debug)]
struct SessionSpawnArgs {
    runtime: sm_core::RuntimeKind,
    role: String,
    dir: Option<PathBuf>,
    namespace: Option<Namespace>,
    labels: Vec<String>,
    agent_config: Option<String>,
    target: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SpawnLocation {
    namespace: Namespace,
    dir: String,
}

fn resolve_spawn_location(
    dir: Option<&PathBuf>,
    namespace: Option<Namespace>,
) -> Result<SpawnLocation> {
    let start_dir = match dir {
        Some(dir) => dir.clone(),
        None => {
            std::env::current_dir().context("cannot read current directory to resolve --dir")?
        }
    };
    let (namespace, canonical_dir) = resolve_namespace_dir(&start_dir, namespace)?.into_pair();

    Ok(SpawnLocation {
        namespace,
        dir: canonical_dir.display().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use super::*;

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    #[test]
    fn explicit_namespace_uses_dir_as_canonical_spawn_dir() {
        let root = tempfile::tempdir().expect("tempdir");
        let project = root.path().join("project");
        std::fs::create_dir_all(&project).expect("project dir");
        let namespace = Namespace::new("alpha").expect("namespace");

        let resolved =
            resolve_spawn_location(Some(&project), Some(namespace.clone())).expect("resolves");

        assert_eq!(resolved.namespace, namespace);
        assert_eq!(
            PathBuf::from(resolved.dir),
            std::fs::canonicalize(project).expect("canonical project")
        );
    }

    #[test]
    fn marker_from_dir_is_ignored_and_dir_is_canonicalized() {
        let root = tempfile::tempdir().expect("tempdir");
        let project = root.path().join("project");
        let child = project.join("child");
        std::fs::create_dir_all(project.join(".sm")).expect("marker parent");
        std::fs::create_dir_all(&child).expect("child dir");
        std::fs::write(project.join(".sm").join("namespace"), "marker").expect("marker");

        let resolved = with_isolated_namespace_env(root.path().join("sm-home"), || {
            resolve_spawn_location(Some(&child), None).expect("resolves dir")
        });

        assert_eq!(resolved.namespace, Namespace::default());
        assert_eq!(
            PathBuf::from(resolved.dir),
            std::fs::canonicalize(child).expect("canonical child")
        );
    }

    #[test]
    fn marker_from_cwd_is_ignored_and_cwd_is_canonicalized() {
        let root = tempfile::tempdir().expect("tempdir");
        let project = root.path().join("project");
        let child = project.join("child");
        std::fs::create_dir_all(project.join(".sm")).expect("marker parent");
        std::fs::create_dir_all(&child).expect("child dir");
        std::fs::write(project.join(".sm").join("namespace"), "cwd-marker").expect("marker");
        let saved = std::env::current_dir().expect("cwd");
        let resolved = with_isolated_namespace_env(root.path().join("sm-home"), || {
            std::env::set_current_dir(&child).expect("chdir");
            let resolved = resolve_spawn_location(None, None).expect("resolves cwd");
            std::env::set_current_dir(saved).expect("restore cwd");
            resolved
        });
        assert_eq!(resolved.namespace, Namespace::default());
        assert_eq!(
            PathBuf::from(resolved.dir),
            std::fs::canonicalize(child).expect("canonical child")
        );
    }

    #[test]
    fn falls_back_to_default_namespace() {
        let project = tempfile::tempdir().expect("tempdir");

        let resolved = with_isolated_namespace_env(project.path().join("sm-home"), || {
            let project = project.path().to_path_buf();
            resolve_spawn_location(Some(&project), None).expect("resolves default")
        });

        assert_eq!(resolved.namespace, Namespace::default());
        assert_eq!(
            PathBuf::from(resolved.dir),
            std::fs::canonicalize(project.path()).expect("canonical project")
        );
    }

    fn with_isolated_namespace_env<T>(sm_home: PathBuf, test: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock");
        let original_sm_home = std::env::var_os("SM_HOME");
        let original_sm_namespace = std::env::var_os("SM_NAMESPACE");

        unsafe {
            std::env::set_var("SM_HOME", sm_home);
            std::env::remove_var("SM_NAMESPACE");
        }
        let result = test();
        restore_env("SM_HOME", original_sm_home);
        restore_env("SM_NAMESPACE", original_sm_namespace);
        result
    }

    fn restore_env(name: &str, value: Option<std::ffi::OsString>) {
        match value {
            Some(value) => unsafe { std::env::set_var(name, value) },
            None => unsafe { std::env::remove_var(name) },
        }
    }
}
