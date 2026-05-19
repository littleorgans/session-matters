use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use lilo_im_core::Principal;
use lilo_rm_client::RuntimeClient;
use lilo_rm_core::{Lifecycle, LifecycleState, StatusFilter};
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use sm_core::{
    DeleteRequest, RpcRequest, RpcResponse, RuntimeKind, Selector, Session, SessionState,
    SpawnRequest,
};
use sm_daemon::handler::DaemonState;
use sm_daemon::identity_client::{IdentityClient, RequestContext};
use sm_driver::RtmdDriver;
use sm_store::SqliteStore;
use uuid::Uuid;

#[tokio::test]
async fn rtmd_driver_spawn_is_visible_to_sm_and_rtmd() {
    let Some(rtm) = rtm_binary() else {
        eprintln!("skipping rtmd integration test; set RTM_TEST_BIN to an rtm binary");
        return;
    };
    let temp = tempfile::tempdir().expect("tempdir");
    write_fake_runtime(temp.path(), "claude");
    let mut rtmd = RtmdHarness::start(&rtm, temp.path());

    let identity = IdentityClient::connect(&temp.path().join("audit.sqlite"), 42)
        .await
        .expect("identity connects");
    let state = DaemonState::new(
        SqliteStore::open_in_memory().expect("store opens"),
        std::sync::Arc::new(RtmdDriver::new(rtmd.socket.clone())),
        std::sync::Arc::new(identity),
    );
    let context = RequestContext::new(Principal::Local(42));

    let session = spawn_session(&state, context, temp.path()).await;
    assert_eq!(session.runtime, RuntimeKind::Claude);
    assert_eq!(session.runtime_pid, runtime_pid(&rtmd, session.id).await);

    rtmd.stop();
}

#[tokio::test]
async fn rtmd_driver_delete_signalled_session_marks_terminated() {
    let Some(rtm) = rtm_binary() else {
        eprintln!("skipping rtmd integration test; set RTM_TEST_BIN to an rtm binary");
        return;
    };
    let temp = tempfile::tempdir().expect("tempdir");
    write_fake_runtime(temp.path(), "claude");
    let mut rtmd = RtmdHarness::start(&rtm, temp.path());
    let state = rtmd_state(&rtmd, temp.path()).await;
    let context = RequestContext::new(Principal::Local(42));
    let session = spawn_session(&state, context.clone(), temp.path()).await;

    let deleted = delete_session(&state, context, session.id, 2).await;

    assert_eq!(deleted.state, SessionState::Terminated);
    assert!(matches!(
        runtime_lifecycle(&rtmd, session.id).await.state,
        LifecycleState::Exited(_)
    ));

    rtmd.stop();
}

#[tokio::test]
async fn rtmd_driver_delete_already_exited_session_marks_terminated() {
    let Some(rtm) = rtm_binary() else {
        eprintln!("skipping rtmd integration test; set RTM_TEST_BIN to an rtm binary");
        return;
    };
    let temp = tempfile::tempdir().expect("tempdir");
    write_fake_runtime(temp.path(), "claude");
    let mut rtmd = RtmdHarness::start(&rtm, temp.path());
    let state = rtmd_state(&rtmd, temp.path()).await;
    let context = RequestContext::new(Principal::Local(42));
    let session = spawn_session(&state, context.clone(), temp.path()).await;

    kill(Pid::from_raw(session.runtime_pid as i32), Signal::SIGKILL)
        .expect("runtime process can be killed");
    wait_for_runtime_exit(&rtmd, session.id).await;
    let deleted = delete_session(&state, context, session.id, 2).await;

    assert_eq!(deleted.state, SessionState::Terminated);

    rtmd.stop();
}

async fn rtmd_state(rtmd: &RtmdHarness, dir: &Path) -> DaemonState {
    let identity = IdentityClient::connect(&dir.join("audit.sqlite"), 42)
        .await
        .expect("identity connects");
    DaemonState::new(
        SqliteStore::open_in_memory().expect("store opens"),
        std::sync::Arc::new(RtmdDriver::new(rtmd.socket.clone())),
        std::sync::Arc::new(identity),
    )
}

async fn spawn_session(state: &DaemonState, context: RequestContext, workspace: &Path) -> Session {
    let result = state
        .handle(
            context,
            RpcRequest::Spawn {
                request: SpawnRequest {
                    runtime: RuntimeKind::Claude,
                    role: "engineer".to_string(),
                    workspace: workspace.display().to_string(),
                    agent_config: None,
                    labels: Vec::new(),
                },
            },
        )
        .await;
    let RpcResponse::Spawned { response } = result.response else {
        panic!("expected spawn response");
    };
    response.session
}

async fn delete_session(
    state: &DaemonState,
    context: RequestContext,
    id: Uuid,
    grace_secs: u64,
) -> Session {
    let deleted = state
        .handle(
            context,
            RpcRequest::Delete {
                request: DeleteRequest {
                    selector: Selector::Id { id },
                    signal: "SIGTERM".to_string(),
                    grace_secs,
                },
            },
        )
        .await;
    let RpcResponse::Deleted { response } = deleted.response else {
        panic!("expected delete response");
    };
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response
        .sessions
        .into_iter()
        .next()
        .expect("deleted session")
}

struct RtmdHarness {
    rtm: PathBuf,
    socket: PathBuf,
    child: Child,
}

impl RtmdHarness {
    fn start(rtm: &Path, dir: &Path) -> Self {
        let socket = dir.join("rtm.sock");
        let db = dir.join("rtm.sqlite");
        let home = dir.join("rtm-home");
        let mut child = Command::new(rtm)
            .arg("daemon")
            .arg("start")
            .env("RTM_SOCKET_PATH", &socket)
            .env("RTM_DB_PATH", &db)
            .env("RTM_HOME", &home)
            .env("PATH", test_path(dir))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("rtmd starts");
        wait_for_socket(&socket, &mut child);
        Self {
            rtm: rtm.to_path_buf(),
            socket,
            child,
        }
    }

    fn stop(&mut self) {
        let _ = Command::new(&self.rtm)
            .arg("daemon")
            .arg("stop")
            .env("RTM_SOCKET_PATH", &self.socket)
            .output();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for RtmdHarness {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn runtime_pid(harness: &RtmdHarness, session_id: Uuid) -> u32 {
    let lifecycle = runtime_lifecycle(harness, session_id).await;
    assert!(matches!(
        lifecycle.state,
        LifecycleState::Forking | LifecycleState::Running
    ));
    lifecycle.runtime_pid.expect("runtime pid")
}

async fn runtime_lifecycle(harness: &RtmdHarness, session_id: Uuid) -> Lifecycle {
    let payload = RuntimeClient::new(harness.socket.clone())
        .status(StatusFilter {
            session_id: Some(session_id),
            session_ids: Vec::new(),
            updated_since: None,
            runtime: None,
            state: None,
        })
        .await
        .expect("rtmd status");
    payload
        .lifecycles
        .into_iter()
        .find(|lifecycle| lifecycle.session_id == session_id)
        .expect("rtmd lifecycle exists")
}

async fn wait_for_runtime_exit(harness: &RtmdHarness, session_id: Uuid) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if matches!(
            runtime_lifecycle(harness, session_id).await.state,
            LifecycleState::Exited(_)
        ) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("runtime lifecycle did not exit");
}

fn rtm_binary() -> Option<PathBuf> {
    std::env::var_os("RTM_TEST_BIN")
        .map(PathBuf::from)
        .or_else(|| {
            let sibling = helioy_root().join("runtime-matters/target/debug/rtm");
            sibling.exists().then_some(sibling)
        })
}

fn helioy_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("workspace has helioy root ancestor")
        .to_path_buf()
}

fn write_fake_runtime(dir: &Path, name: &str) {
    let path = dir.join(name);
    std::fs::write(
        &path,
        "#!/bin/sh\nprintf 'rtm fake runtime ready\\n'\nexec sleep 60\n",
    )
    .expect("fake runtime writes");
    let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).expect("permissions");
}

fn test_path(dir: &Path) -> String {
    let current = std::env::var_os("PATH").unwrap_or_default();
    let paths = std::iter::once(dir.to_path_buf()).chain(std::env::split_paths(&current));
    std::env::join_paths(paths)
        .expect("joined path")
        .to_string_lossy()
        .into_owned()
}

fn wait_for_socket(socket: &Path, child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if UnixStream::connect(socket).is_ok() {
            return;
        }
        if child.try_wait().expect("rtmd try_wait").is_some() {
            panic!("rtmd exited before socket appeared");
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("rtmd socket never appeared at {}", socket.display());
}
