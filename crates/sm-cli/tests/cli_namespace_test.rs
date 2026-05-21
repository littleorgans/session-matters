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
fn init_command_is_rejected_by_clap() {
    let daemon = common::DaemonFixture::start();

    let output = daemon
        .command()
        .arg("init")
        .output()
        .expect("sm init executes");

    assert!(!output.status.success());
    assert!(stderr(&output).contains("unrecognized subcommand 'init'"));
}

#[test]
fn init_namespace_command_is_rejected_by_clap() {
    let daemon = common::DaemonFixture::start();

    let output = daemon
        .command()
        .args(["init", "namespace", "alpha"])
        .output()
        .expect("sm init namespace executes");

    assert!(!output.status.success());
    assert!(stderr(&output).contains("unrecognized subcommand 'init'"));
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
