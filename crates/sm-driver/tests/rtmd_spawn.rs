use std::path::PathBuf;

use lilo_rm_core::{
    LaunchEnv, Lifecycle, LifecycleState, LostEvidence, RuntimeEvent, RuntimeKind, RuntimeResponse,
    RuntimeRpc, ShellResume, SpawnRequest, SpawnedPayload, read_json_line, write_json_line,
};
use sm_core::RuntimeKind as SmRuntimeKind;
use sm_driver::{RtmdDriver, SpawnDriver, SpawnLaunch};
use tokio::io::BufReader;
use tokio::net::UnixListener;
use uuid::Uuid;

#[tokio::test]
async fn rtmd_spawn_forwards_env_shell_resume_and_force_enabled() {
    rtmd_spawn_forwards_env_shell_resume_and_force(true).await;
}

#[tokio::test]
async fn rtmd_spawn_forwards_force_disabled() {
    rtmd_spawn_forwards_env_shell_resume_and_force(false).await;
}

async fn rtmd_spawn_forwards_env_shell_resume_and_force(force: bool) {
    let session_id = Uuid::now_v7();
    let tempdir = tempfile::tempdir().expect("tempdir");
    let socket_path = tempdir.path().join("rtmd.sock");
    let listener = UnixListener::bind(&socket_path).expect("bind test socket");
    let driver = RtmdDriver::new(socket_path);
    let shell_resume = ShellResume {
        argv: vec!["/bin/zsh".to_string()],
        env: vec![LaunchEnv::new("TERM", "xterm-256color")],
        cwd: PathBuf::from("/tmp/session"),
    };

    let server = tokio::spawn({
        let shell_resume = shell_resume.clone();
        async move {
            let _tempdir = tempdir;
            let (stream, _) = listener.accept().await.expect("accept client");
            let (read_half, mut write_half) = stream.into_split();
            let mut reader = BufReader::new(read_half);
            let rpc: RuntimeRpc = read_json_line(&mut reader).await.expect("read rpc");
            let RuntimeRpc::Spawn { request } = rpc else {
                panic!("expected spawn rpc");
            };
            assert_eq!(request.env, vec![LaunchEnv::new("HOME", "/Users/tester")]);
            assert_eq!(request.shell_resume, Some(shell_resume));
            assert_eq!(request.force, force);
            write_json_line(&mut write_half, &RuntimeResponse::Spawned(spawned(request)))
                .await
                .expect("write response");
        }
    });

    driver
        .spawn(
            &session_id.to_string(),
            &SpawnLaunch {
                runtime: SmRuntimeKind::Claude,
                cwd: PathBuf::from("/tmp/session"),
                target: "headless".to_string(),
                env: vec![LaunchEnv::new("HOME", "/Users/tester")],
                shell_resume: Some(shell_resume),
                force,
            },
        )
        .await
        .expect("spawn delegates to rtmd");
    server.await.expect("server task");
}

fn spawned(request: SpawnRequest) -> SpawnedPayload {
    let lifecycle = Lifecycle {
        session_id: request.session_id,
        runtime: RuntimeKind::Claude,
        state: LifecycleState::Running,
        shim_pid: None,
        runtime_pid: Some(42),
        start_time: None,
        tmux_pane: None,
        log_availability: None,
    };
    SpawnedPayload {
        lifecycle,
        event: RuntimeEvent::Lost {
            session_id: request.session_id,
            evidence: LostEvidence::PidNotAlive,
        },
        log_dir: None,
        stdout_path: None,
        stderr_path: None,
    }
}
