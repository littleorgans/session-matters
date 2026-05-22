mod common;

use std::path::Path;

use serde_json::Value;

#[test]
fn get_session_help_exposes_only_session_read_arguments() {
    for resource in ["session", "sessions"] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_sm"))
            .args(["get", resource, "--help"])
            .output()
            .expect("sm get session help executes");

        assert_success("sm get session help", &output);
        let stdout = stdout(&output);
        assert!(stdout.contains("--selector"));
        assert!(stdout.contains("--namespace"));
        assert!(stdout.contains("--all-namespaces"));
        assert!(stdout.contains("--json"));
    }
}

#[test]
fn get_namespace_help_exposes_only_namespace_read_arguments() {
    for resource in ["namespace", "namespaces"] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_sm"))
            .args(["get", resource, "--help"])
            .output()
            .expect("sm get namespace help executes");

        assert_success("sm get namespace help", &output);
        let stdout = stdout(&output);
        assert!(stdout.contains("--json"));
        assert!(!stdout.contains("--selector"));
        assert!(!stdout.contains("--namespace <NAMESPACE>"));
        assert!(!stdout.contains("--all-namespaces"));
    }
}

#[test]
fn create_help_lists_namespace_and_session_resources() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_sm"))
        .args(["create", "--help"])
        .output()
        .expect("sm create help executes");

    assert_success("sm create --help", &output);
    let stdout = stdout(&output);
    assert!(stdout.contains("namespace"));
    assert!(stdout.contains("session"));
}

#[test]
fn create_session_help_exposes_only_declarative_arguments() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_sm"))
        .args(["create", "session", "--help"])
        .output()
        .expect("sm create session help executes");

    assert_success("sm create session --help", &output);
    let stdout = stdout(&output);
    assert!(stdout.contains("<RUNTIME>"));
    assert!(stdout.contains("--role"));
    assert!(stdout.contains("--dir"));
    assert!(stdout.contains("--namespace"));
    assert!(stdout.contains("--label"));
    assert!(stdout.contains("--agent-config"));
    assert!(!stdout.contains("--target"));
    assert!(!stdout.contains("--detach"));
    assert!(!stdout.contains("--force"));
}

#[test]
fn run_help_exposes_force_as_imperative_argument() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_sm"))
        .args(["run", "--help"])
        .output()
        .expect("sm run --help executes");

    assert_success("sm run --help", &output);
    let stdout = stdout(&output);
    assert!(stdout.contains("--force"));
    assert!(stdout.contains("Preempt an occupied tmux pane"));
}

#[test]
fn create_session_persists_headless_record_without_foreground_attach() {
    let runtime_path = common::fake_runtime_path("claude");
    let daemon = common::DaemonFixture::start_with_runtime_path(runtime_path.path());
    let project = daemon.dir.path().join("project");
    std::fs::create_dir_all(&project).expect("project dir");

    let created = daemon
        .command()
        .args([
            "create",
            "session",
            "claude",
            "--role",
            "engineer",
            "--dir",
            &project.display().to_string(),
            "--label",
            "area=create",
        ])
        .output()
        .expect("sm create session executes");
    assert_success("sm create session", &created);
    let id = first_field(&created.stdout);

    let single = daemon
        .command()
        .args(["get", "session", &id, "--json"])
        .output()
        .expect("sm get session <id> --json executes");
    assert_success("sm get session <id> --json", &single);
    let session: Value = serde_json::from_slice(&single.stdout).expect("session JSON parses");
    let canonical_project = canonical_display(&project);
    assert_eq!(session["id"], id);
    assert_eq!(session["runtime"], "claude");
    assert_eq!(session["role"], "engineer");
    assert_eq!(session["namespace"], "default");
    assert_eq!(session["dir"], canonical_project);
    assert_eq!(session["workspace"], canonical_project);
    assert_eq!(session["state"], "RUNNING");
    assert_eq!(session["tmux_pane"], Value::Null);
    assert_eq!(session["labels"][0]["key"], "area");
    assert_eq!(session["labels"][0]["value"], "create");
}

#[test]
fn create_session_and_run_persist_compatible_records_for_shared_inputs() {
    let runtime_path = common::fake_runtime_path("claude");
    let daemon = common::DaemonFixture::start_with_runtime_path(runtime_path.path());
    let project = daemon.dir.path().join("project");
    std::fs::create_dir_all(&project).expect("project dir");

    let run = daemon
        .command()
        .args([
            "run",
            "claude",
            "--role",
            "engineer",
            "--dir",
            &project.display().to_string(),
            "--label",
            "area=shared",
            "--detach",
        ])
        .output()
        .expect("sm run executes");
    assert_success("sm run", &run);

    let created = daemon
        .command()
        .args([
            "create",
            "session",
            "claude",
            "--role",
            "engineer",
            "--dir",
            &project.display().to_string(),
            "--label",
            "area=shared",
        ])
        .output()
        .expect("sm create session executes");
    assert_success("sm create session", &created);

    let run_session = get_session_json(&daemon, &first_field(&run.stdout));
    let create_session = get_session_json(&daemon, &first_field(&created.stdout));
    for field in ["runtime", "role", "namespace", "dir", "workspace", "labels"] {
        assert_eq!(create_session[field], run_session[field], "{field} differs");
    }
    assert_eq!(create_session["tmux_pane"], run_session["tmux_pane"]);
    assert_eq!(create_session["agent_config"], run_session["agent_config"]);
}

#[test]
fn removed_get_forms_are_rejected_by_clap() {
    for args in [
        ["get", "agent", "--help"].as_slice(),
        ["get", "agents", "--help"].as_slice(),
        ["get", "namespace", "--selector", "all"].as_slice(),
        ["get", "namespaces", "default"].as_slice(),
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_sm"))
            .args(args)
            .output()
            .expect("sm get rejected form executes");

        assert!(!output.status.success());
    }
}

#[test]
fn session_resources_list_and_get_by_id() {
    let runtime_path = common::fake_runtime_path("claude");
    let daemon = common::DaemonFixture::start_with_runtime_path(runtime_path.path());

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

    let singular_list = daemon
        .command()
        .args(["get", "session"])
        .output()
        .expect("sm get session executes");
    assert_success("sm get session", &singular_list);
    assert_table_contains(&singular_list.stdout, &id);

    let plural_list = daemon
        .command()
        .args(["get", "sessions"])
        .output()
        .expect("sm get sessions executes");
    assert_success("sm get sessions", &plural_list);
    assert_table_contains(&plural_list.stdout, &id);

    let selected_list = daemon
        .command()
        .args(["get", "session", "--selector", "all"])
        .output()
        .expect("sm get session --selector all executes");
    assert_success("sm get session --selector all", &selected_list);
    assert_table_contains(&selected_list.stdout, &id);

    let json_list = daemon
        .command()
        .args(["get", "session", "--json"])
        .output()
        .expect("sm get session --json executes");
    assert_success("sm get session --json", &json_list);
    let sessions: Value = serde_json::from_slice(&json_list.stdout).expect("list JSON parses");
    assert!(sessions.as_array().is_some_and(|items| !items.is_empty()));

    let single = daemon
        .command()
        .args(["get", "session", &id])
        .output()
        .expect("sm get session <id> executes");
    assert_success("sm get session <id>", &single);
    let stdout = String::from_utf8_lossy(&single.stdout);
    assert!(stdout.contains(&id));
    assert!(!stdout.starts_with("ID RUNTIME"));
}

#[test]
fn run_persists_canonical_dir_from_cli_resolution() {
    let runtime_path = common::fake_runtime_path("claude");
    let daemon = common::DaemonFixture::start_with_runtime_path(runtime_path.path());
    let project = daemon.dir.path().join("project");
    std::fs::create_dir_all(&project).expect("project dir");

    let run = daemon
        .command()
        .current_dir(&project)
        .args([
            "run", "claude", "--role", "engineer", "--dir", ".", "--detach",
        ])
        .output()
        .expect("sm run executes");
    assert_success("sm run --dir", &run);
    let id = first_field(&run.stdout);

    let single = daemon
        .command()
        .args(["get", "session", &id, "--json"])
        .output()
        .expect("sm get session <id> --json executes");
    assert_success("sm get session <id> --json", &single);
    let session: Value = serde_json::from_slice(&single.stdout).expect("session JSON parses");
    let canonical_project = canonical_display(&project);
    assert_eq!(session["dir"], canonical_project);
    assert_eq!(session["workspace"], canonical_project);
    assert_eq!(session["namespace"], "default");
}

#[test]
fn workspace_arg_is_rejected_by_clap() {
    let daemon = common::DaemonFixture::start();

    let run = daemon
        .command()
        .args([
            "run",
            "claude",
            "--role",
            "engineer",
            "--dir",
            &daemon.dir.path().display().to_string(),
            "--workspace",
            &daemon.dir.path().display().to_string(),
        ])
        .output()
        .expect("sm run executes");

    assert!(!run.status.success());
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(stderr.contains("unexpected argument '--workspace'"));
    assert!(!stderr.contains("--workspace is deprecated"));
}

#[test]
fn unknown_namespace_error_is_surfaced_from_daemon() {
    let runtime_path = common::fake_runtime_path("claude");
    let daemon = common::DaemonFixture::start_with_runtime_path(runtime_path.path());

    let run = daemon
        .command()
        .args([
            "run",
            "claude",
            "--role",
            "engineer",
            "--dir",
            &daemon.dir.path().display().to_string(),
            "--namespace",
            "missing",
            "--detach",
        ])
        .output()
        .expect("sm run executes");

    assert!(!run.status.success());
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(stderr.contains("namespace not found: missing"));
}

fn get_session_json(daemon: &common::DaemonFixture, id: &str) -> Value {
    let output = daemon
        .command()
        .args(["get", "session", id, "--json"])
        .output()
        .expect("sm get session <id> --json executes");
    assert_success("sm get session <id> --json", &output);
    serde_json::from_slice(&output.stdout).expect("session JSON parses")
}

fn assert_success(command: &str, output: &std::process::Output) {
    assert!(
        output.status.success(),
        "{command} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_table_contains(stdout: &[u8], id: &str) {
    let stdout = String::from_utf8_lossy(stdout);
    assert!(stdout.starts_with("ID RUNTIME"));
    assert!(stdout.contains(id));
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn first_field(stdout: &[u8]) -> String {
    String::from_utf8_lossy(stdout)
        .split_whitespace()
        .next()
        .expect("stdout has first field")
        .to_string()
}

fn canonical_display(path: &Path) -> Value {
    Value::String(
        std::fs::canonicalize(path)
            .expect("canonical path")
            .display()
            .to_string(),
    )
}
