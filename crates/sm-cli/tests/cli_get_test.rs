mod common;

use serde_json::Value;

#[test]
fn singular_agent_resource_lists_without_id_and_gets_with_id() {
    let runtime_path = common::fake_runtime_path("claude");
    let daemon = common::DaemonFixture::start_with_runtime_path(runtime_path.path());

    let run = daemon
        .command()
        .args([
            "run",
            "claude",
            "--role",
            "engineer",
            "--workspace",
            "session-matters",
            "--detach",
        ])
        .output()
        .expect("sm run executes");
    assert_success("sm run", &run);
    let id = first_field(&run.stdout);

    let singular_list = daemon
        .command()
        .args(["get", "agent"])
        .output()
        .expect("sm get agent executes");
    assert_success("sm get agent", &singular_list);
    assert_table_contains(&singular_list.stdout, &id);

    let plural_list = daemon
        .command()
        .args(["get", "agents"])
        .output()
        .expect("sm get agents executes");
    assert_success("sm get agents", &plural_list);
    assert_table_contains(&plural_list.stdout, &id);

    let selected_list = daemon
        .command()
        .args(["get", "agent", "--selector", "all"])
        .output()
        .expect("sm get agent --selector all executes");
    assert_success("sm get agent --selector all", &selected_list);
    assert_table_contains(&selected_list.stdout, &id);

    let json_list = daemon
        .command()
        .args(["get", "agent", "--json"])
        .output()
        .expect("sm get agent --json executes");
    assert_success("sm get agent --json", &json_list);
    let sessions: Value = serde_json::from_slice(&json_list.stdout).expect("list JSON parses");
    assert!(sessions.as_array().is_some_and(|items| !items.is_empty()));

    let single = daemon
        .command()
        .args(["get", "agent", &id])
        .output()
        .expect("sm get agent <id> executes");
    assert_success("sm get agent <id>", &single);
    let stdout = String::from_utf8_lossy(&single.stdout);
    assert!(stdout.contains(&id));
    assert!(!stdout.starts_with("ID RUNTIME"));
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

fn first_field(stdout: &[u8]) -> String {
    String::from_utf8_lossy(stdout)
        .split_whitespace()
        .next()
        .expect("stdout has first field")
        .to_string()
}
