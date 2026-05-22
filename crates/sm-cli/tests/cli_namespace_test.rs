mod common;

use serde_json::Value;

#[test]
fn create_and_get_namespace_support_human_and_json_output() {
    let daemon = common::DaemonFixture::start();

    let created = daemon
        .command()
        .args(["create", "namespace", "alpha"])
        .output()
        .expect("sm create namespace executes");
    assert_success("sm create namespace", &created);
    assert!(stdout(&created).contains("created namespace: alpha"));

    let recreated = daemon
        .command()
        .args(["create", "namespace", "alpha"])
        .output()
        .expect("sm create namespace executes");
    assert_success("sm create namespace again", &recreated);
    assert!(stdout(&recreated).contains("namespace already exists: alpha"));

    let listed = daemon
        .command()
        .args(["get", "namespace"])
        .output()
        .expect("sm get namespace executes");
    assert_success("sm get namespace", &listed);
    assert!(stdout(&listed).contains("NAMESPACE CREATED_AT"));
    assert!(stdout(&listed).contains("alpha"));
    assert!(stdout(&listed).contains("default"));

    let plural_listed = daemon
        .command()
        .args(["get", "namespaces"])
        .output()
        .expect("sm get namespaces executes");
    assert_success("sm get namespaces", &plural_listed);
    assert!(stdout(&plural_listed).contains("alpha"));
    assert!(stdout(&plural_listed).contains("default"));

    let single = daemon
        .command()
        .args(["get", "namespace", "alpha"])
        .output()
        .expect("sm get namespace alpha executes");
    assert_success("sm get namespace alpha", &single);
    assert!(stdout(&single).contains("NAMESPACE CREATED_AT"));
    assert!(stdout(&single).contains("alpha"));
    assert!(!stdout(&single).contains("default"));

    let alias_single = daemon
        .command()
        .args(["get", "namespaces", "alpha"])
        .output()
        .expect("sm get namespaces alpha executes");
    assert_success("sm get namespaces alpha", &alias_single);
    assert!(stdout(&alias_single).contains("alpha"));
    assert!(!stdout(&alias_single).contains("default"));

    let json = daemon
        .command()
        .args(["get", "namespace", "--json"])
        .output()
        .expect("sm get namespace --json executes");
    assert_success("sm get namespace --json", &json);
    let namespaces: Value = serde_json::from_slice(&json.stdout).expect("namespace JSON parses");
    assert_eq!(namespaces[0]["namespace"], "alpha");
    assert_eq!(namespaces[1]["namespace"], "default");
}

#[test]
fn create_namespace_rejects_default() {
    let daemon = common::DaemonFixture::start();

    let created = daemon
        .command()
        .args(["create", "namespace", "default"])
        .output()
        .expect("sm create namespace executes");

    assert!(!created.status.success());
    assert!(stderr(&created).contains("namespace name is reserved: default"));
}

#[test]
fn init_command_is_rejected_by_clap() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_sm"))
        .arg("init")
        .output()
        .expect("sm init executes");

    assert!(!output.status.success());
    assert!(stderr(&output).contains("unrecognized subcommand 'init'"));
}

#[test]
fn init_namespace_command_is_rejected_by_clap() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_sm"))
        .args(["init", "namespace", "alpha"])
        .output()
        .expect("sm init namespace executes");

    assert!(!output.status.success());
    assert!(stderr(&output).contains("unrecognized subcommand 'init'"));
}

#[test]
fn delete_namespace_help_does_not_expose_session_flags() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_sm"))
        .args(["delete", "namespace", "--help"])
        .output()
        .expect("sm delete namespace --help executes");

    assert_success("sm delete namespace --help", &output);
    let stdout = stdout(&output);
    assert!(!stdout.contains("--signal"));
    assert!(!stdout.contains("--grace"));
}

#[test]
fn delete_namespace_rejects_default_before_daemon_connect() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_sm"))
        .args(["delete", "namespace", "default"])
        .output()
        .expect("sm delete namespace default executes");

    assert!(!output.status.success());
    assert!(stderr(&output).contains("namespace name is reserved: default"));
}

#[test]
fn delete_namespace_cascades_sessions_and_clears_binding() {
    let runtime_path = common::fake_runtime_path("claude");
    let daemon = common::DaemonFixture::start_with_runtime_path(runtime_path.path());
    create_namespace(&daemon, "foo");
    set_context(&daemon, "foo");

    let run = daemon
        .command()
        .args([
            "run",
            "claude",
            "--role",
            "engineer",
            "--dir",
            &daemon.dir.path().display().to_string(),
            "--detach",
        ])
        .output()
        .expect("sm run executes");
    assert_success("sm run", &run);
    let id = first_field(&run.stdout);

    let deleted = daemon
        .command()
        .args(["delete", "namespace", "foo"])
        .output()
        .expect("sm delete namespace executes");
    assert_success("sm delete namespace foo", &deleted);
    assert!(stdout(&deleted).contains("deleted namespace: foo"));
    assert!(!daemon.dir.path().join("namespace").exists());

    let listed = daemon
        .command()
        .args(["get", "namespace"])
        .output()
        .expect("sm get namespace executes");
    assert_success("sm get namespace", &listed);
    assert!(!stdout(&listed).contains("foo"));

    let sessions = daemon
        .command()
        .args(["get", "session", "-A"])
        .output()
        .expect("sm get session -A executes");
    assert_success("sm get session -A", &sessions);
    assert!(!stdout(&sessions).contains(&id));
}

#[test]
fn delete_namespace_clears_stale_binding_when_catalog_entry_is_absent() {
    let daemon = common::DaemonFixture::start();
    std::fs::write(daemon.dir.path().join("namespace"), "missing\n").expect("binding writes");

    let output = daemon
        .command()
        .args(["delete", "namespace", "missing"])
        .output()
        .expect("sm delete namespace missing executes");

    assert_success("sm delete namespace missing", &output);
    assert!(stdout(&output).contains("catalog entry already absent; stale binding cleared"));
    assert!(!daemon.dir.path().join("namespace").exists());
}

#[test]
fn delete_namespace_surfaces_binding_clear_failure_and_retry_converges() {
    let daemon = common::DaemonFixture::start();
    create_namespace(&daemon, "foo");
    set_context(&daemon, "foo");

    let failed = daemon
        .command()
        .env("SM_FAULT_NAMESPACE_BINDING_CLEAR", "1")
        .args(["delete", "namespace", "foo"])
        .output()
        .expect("sm delete namespace foo executes");
    assert!(!failed.status.success());
    assert!(stderr(&failed).contains("failed to clear namespace binding"));
    assert_eq!(binding_contents(daemon.dir.path()), "foo\n");

    let retry = daemon
        .command()
        .args(["delete", "namespace", "foo"])
        .output()
        .expect("sm delete namespace foo retry executes");
    assert_success("sm delete namespace foo retry", &retry);
    assert!(stdout(&retry).contains("catalog entry already absent; stale binding cleared"));
    assert!(!daemon.dir.path().join("namespace").exists());
}

#[test]
fn delete_namespace_daemon_unreachable_does_not_clear_binding() {
    let sm_home = tempfile::tempdir().expect("sm home");
    std::fs::write(sm_home.path().join("namespace"), "foo\n").expect("binding writes");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_sm"))
        .args(["delete", "namespace", "foo"])
        .env("SM_HOME", sm_home.path())
        .env("HOME", sm_home.path())
        .output()
        .expect("sm delete namespace foo executes");

    assert!(!output.status.success());
    assert!(stderr(&output).contains("failed to connect"));
    assert_eq!(binding_contents(sm_home.path()), "foo\n");
}

fn create_namespace(daemon: &common::DaemonFixture, name: &str) {
    let output = daemon
        .command()
        .args(["create", "namespace", name])
        .output()
        .expect("sm create namespace executes");
    assert_success("sm create namespace", &output);
}

fn set_context(daemon: &common::DaemonFixture, name: &str) {
    let output = daemon
        .command()
        .args(["config", "set-context", name])
        .output()
        .expect("sm config set-context executes");
    assert_success("sm config set-context", &output);
}

fn binding_contents(dir: &std::path::Path) -> String {
    std::fs::read_to_string(dir.join("namespace")).expect("binding file reads")
}

fn first_field(stdout: &[u8]) -> String {
    String::from_utf8_lossy(stdout)
        .split_whitespace()
        .next()
        .expect("stdout has first field")
        .to_string()
}

fn assert_success(command: &str, output: &std::process::Output) {
    assert!(
        output.status.success(),
        "{command} failed\nstdout:\n{}\nstderr:\n{}",
        stdout(output),
        stderr(output)
    );
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}
