use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use lilo_im_core::Principal;
use lilo_rm_client::RuntimeClient;
use lilo_rm_core::{LifecycleState, StatusFilter};
use sm_core::{RpcRequest, RpcResponse, RuntimeKind, SpawnRequest};
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

    let result = state
        .handle(
            RequestContext::new(Principal::Local(42)),
            RpcRequest::Spawn {
                request: SpawnRequest {
                    runtime: RuntimeKind::Claude,
                    role: "engineer".to_string(),
                    workspace: temp.path().display().to_string(),
                    agent_config: None,
                    labels: Vec::new(),
                },
            },
        )
        .await;
    let RpcResponse::Spawned { response } = result.response else {
        panic!("expected spawn response");
    };
    assert_eq!(response.session.runtime, RuntimeKind::Claude);
    assert_eq!(
        response.session.runtime_pid,
        runtime_pid(&rtmd, response.session.id).await
    );

    rtmd.stop();
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
    let lifecycle = payload
        .lifecycles
        .into_iter()
        .find(|lifecycle| lifecycle.session_id == session_id)
        .expect("rtmd lifecycle exists");
    assert!(matches!(
        lifecycle.state,
        LifecycleState::Forking | LifecycleState::Running
    ));
    lifecycle.runtime_pid.expect("runtime pid")
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
