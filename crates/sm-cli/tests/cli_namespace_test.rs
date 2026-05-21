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
fn init_namespace_creates_record_and_marker() {
    let daemon = common::DaemonFixture::start();
    let project = daemon.dir.path().join("project");
    std::fs::create_dir(&project).expect("project dir creates");
    let project_arg = project.display().to_string();

    let initialized = daemon
        .command()
        .args(["init", "namespace", "alpha", "--dir", &project_arg])
        .output()
        .expect("sm init namespace executes");
    assert_success("sm init namespace", &initialized);
    assert!(stdout(&initialized).contains("created namespace: alpha"));

    let marker = project.join(".sm").join("namespace");
    assert_eq!(
        std::fs::read_to_string(marker).expect("marker reads"),
        "alpha\n"
    );

    let got = daemon
        .command()
        .args(["get", "namespace", "alpha", "--json"])
        .output()
        .expect("sm get namespace alpha executes");
    assert_success("sm get namespace alpha", &got);
    let namespace: Value = serde_json::from_slice(&got.stdout).expect("namespace JSON parses");
    assert_eq!(namespace["namespace"], "alpha");
}

#[test]
fn init_namespace_refuses_marker_conflict_before_create() {
    let daemon = common::DaemonFixture::start();
    let project = daemon.dir.path().join("project");
    let marker_dir = project.join(".sm");
    std::fs::create_dir_all(&marker_dir).expect("marker dir creates");
    std::fs::write(marker_dir.join("namespace"), "beta\n").expect("marker writes");
    let project_arg = project.display().to_string();

    let initialized = daemon
        .command()
        .args(["init", "namespace", "alpha", "--dir", &project_arg])
        .output()
        .expect("sm init namespace executes");
    assert!(!initialized.status.success());
    assert!(stderr(&initialized).contains("namespace marker already exists"));

    let got = daemon
        .command()
        .args(["get", "namespace", "alpha"])
        .output()
        .expect("sm get namespace alpha executes");
    assert!(!got.status.success());
    assert!(stderr(&got).contains("unknown namespace: alpha"));
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
