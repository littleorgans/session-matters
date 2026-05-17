use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use sm_core::{MailCheckRequest, RpcRequest, RpcResponse, Selector};
use uuid::Uuid;

#[path = "../tests/common/mod.rs"]
mod common;

const MAIL_CHECK_BUDGET: Duration = Duration::from_millis(50);
const RPC_BUDGET: Duration = Duration::from_millis(5);

fn hot_path_benches(c: &mut Criterion) {
    let runtime_dir = fake_runtime_dir();
    let daemon = common::DaemonFixture::start_with_runtime_path(runtime_dir.path());
    let session_id = spawn_bench_agent(&daemon);
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime starts");

    assert_budget("sm mail check cold-start", MAIL_CHECK_BUDGET, || {
        run_mail_check(&daemon, &session_id);
    });
    assert_budget("daemon RPC round-trip", RPC_BUDGET, || {
        run_rpc_round_trip(&runtime, &daemon, session_id);
    });

    c.bench_function("sm mail check cold-start", |bench| {
        bench.iter(|| black_box(run_mail_check(&daemon, &session_id)));
    });
    c.bench_function("daemon RPC round-trip", |bench| {
        bench.iter(|| black_box(run_rpc_round_trip(&runtime, &daemon, session_id)));
    });
}

fn assert_budget<F>(name: &str, budget: Duration, mut run: F)
where
    F: FnMut(),
{
    let mut samples = (0..5)
        .map(|_| {
            let started = Instant::now();
            run();
            started.elapsed()
        })
        .collect::<Vec<_>>();
    samples.sort();
    let median = samples[samples.len() / 2];
    assert!(
        median <= budget,
        "{name} median {median:?} exceeds budget {budget:?}"
    );
}

fn run_mail_check(daemon: &common::DaemonFixture, session_id: &Uuid) -> usize {
    let output = daemon
        .command()
        .args(["mail", "check", "--selector", &session_id.to_string()])
        .output()
        .expect("sm mail check runs");
    assert!(
        output.status.success(),
        "sm mail check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    assert_eq!(stdout.trim(), "0 unread");
    0
}

fn run_rpc_round_trip(
    runtime: &tokio::runtime::Runtime,
    daemon: &common::DaemonFixture,
    session_id: Uuid,
) -> usize {
    let request = RpcRequest::MailCheck {
        request: MailCheckRequest {
            selector: Selector::Id { id: session_id },
        },
    };
    let response = runtime
        .block_on(sm_daemon::send_request(&daemon.socket_path(), &request))
        .expect("daemon RPC succeeds");
    match response {
        RpcResponse::MailChecked { response } => response.unread,
        other => panic!("unexpected daemon response: {other:?}"),
    }
}

fn spawn_bench_agent(daemon: &common::DaemonFixture) -> Uuid {
    let output = daemon
        .command()
        .args([
            "run",
            "codex",
            "--role",
            "bench",
            "--workspace",
            "bench",
            "--detach",
        ])
        .output()
        .expect("sm run starts");
    assert!(
        output.status.success(),
        "sm run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf8");
    let id = stdout
        .split_whitespace()
        .next()
        .expect("session id is printed");
    Uuid::parse_str(id).expect("session id is a uuid")
}

fn fake_runtime_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("fake runtime dir creates");
    let runtime = dir.path().join("codex");
    fs::write(
        &runtime,
        "#!/bin/sh\ntrap 'exit 0' TERM INT\nwhile true; do sleep 1; done\n",
    )
    .expect("fake runtime writes");
    let mut permissions = fs::metadata(&runtime)
        .expect("fake runtime metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&runtime, permissions).expect("fake runtime executable");
    dir
}

criterion_group!(benches, hot_path_benches);
criterion_main!(benches);
