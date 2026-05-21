mod common;

use std::path::Path;

#[test]
fn namespace_scope_applies_to_selector_consuming_cli_surfaces() {
    let runtime_path = common::fake_runtime_path("claude");
    let daemon = common::DaemonFixture::start_with_runtime_path(runtime_path.path());
    let alpha_dir = daemon.dir.path().join("alpha");
    let beta_dir = daemon.dir.path().join("beta");
    std::fs::create_dir_all(&alpha_dir).expect("alpha dir");
    std::fs::create_dir_all(&beta_dir).expect("beta dir");
    init_namespace(&daemon, "alpha", &alpha_dir);
    init_namespace(&daemon, "beta", &beta_dir);

    let alpha_id = run_session(&daemon, "alpha", &alpha_dir);
    let beta_id = run_session(&daemon, "beta", &beta_dir);

    let default_scoped = daemon
        .command()
        .current_dir(&alpha_dir)
        .args(["get", "agents"])
        .output()
        .expect("sm get agents executes");
    assert_success("sm get agents", &default_scoped);
    assert_contains_only(&default_scoped, &alpha_id, &beta_id);

    let explicit_namespace = daemon
        .command()
        .current_dir(&alpha_dir)
        .args(["get", "agents", "--namespace", "beta"])
        .output()
        .expect("sm get agents --namespace executes");
    assert_success("sm get agents --namespace beta", &explicit_namespace);
    assert_contains_only(&explicit_namespace, &beta_id, &alpha_id);

    let all_namespaces = daemon
        .command()
        .current_dir(&alpha_dir)
        .args(["get", "agents", "-A"])
        .output()
        .expect("sm get agents -A executes");
    assert_success("sm get agents -A", &all_namespaces);
    assert_contains(&all_namespaces, &alpha_id);
    assert_contains(&all_namespaces, &beta_id);

    let namespace_selector = daemon
        .command()
        .current_dir(&alpha_dir)
        .args(["get", "agents", "--selector", "namespace:beta", "-A"])
        .output()
        .expect("sm get agents namespace selector executes");
    assert_success(
        "sm get agents --selector namespace:beta -A",
        &namespace_selector,
    );
    assert_contains_only(&namespace_selector, &beta_id, &alpha_id);

    let namespace_selector_default_scope = daemon
        .command()
        .current_dir(&alpha_dir)
        .args(["nudge", "--to", "namespace:beta", "--content", "ping"])
        .output()
        .expect("sm nudge namespace selector executes");
    assert_success(
        "sm nudge --to namespace:beta",
        &namespace_selector_default_scope,
    );
    assert_total_line_count(&namespace_selector_default_scope, 1);
    assert_contains_only(&namespace_selector_default_scope, &beta_id, &alpha_id);

    let matching_namespace_flag = daemon
        .command()
        .current_dir(&alpha_dir)
        .args([
            "nudge",
            "--to",
            "namespace:beta",
            "--namespace",
            "beta",
            "--content",
            "ping",
        ])
        .output()
        .expect("sm nudge matching namespace flag executes");
    assert_success(
        "sm nudge --to namespace:beta --namespace beta",
        &matching_namespace_flag,
    );
    assert_contains_only(&matching_namespace_flag, &beta_id, &alpha_id);

    let conflicting_namespace_flag = daemon
        .command()
        .current_dir(&alpha_dir)
        .args([
            "nudge",
            "--to",
            "namespace:beta",
            "--namespace",
            "alpha",
            "--content",
            "ping",
        ])
        .output()
        .expect("sm nudge conflicting namespace flag executes");
    assert!(!conflicting_namespace_flag.status.success());
    assert!(stderr(&conflicting_namespace_flag).contains("--namespace alpha"));
    assert!(stderr(&conflicting_namespace_flag).contains("namespace:beta"));

    let beta_dir_selector = format!("dir:{}", canonical(&beta_dir));
    let dir_selector = daemon
        .command()
        .current_dir(&alpha_dir)
        .args(["get", "agents", "--selector", &beta_dir_selector, "-A"])
        .output()
        .expect("sm get agents dir selector executes");
    assert_success("sm get agents --selector dir -A", &dir_selector);
    assert_contains_only(&dir_selector, &beta_id, &alpha_id);

    let mail_default = daemon
        .command()
        .current_dir(&alpha_dir)
        .args(["mail", "send", "--to", "all", "--content", "scoped"])
        .output()
        .expect("sm mail send executes");
    assert_success("sm mail send", &mail_default);
    assert_line_count(&mail_default.stdout, 1);

    let mail_all = daemon
        .command()
        .current_dir(&alpha_dir)
        .args(["mail", "send", "--to", "all", "--content", "all", "-A"])
        .output()
        .expect("sm mail send -A executes");
    assert_success("sm mail send -A", &mail_all);
    assert_line_count(&mail_all.stdout, 2);

    let nudge_default = daemon
        .command()
        .current_dir(&alpha_dir)
        .args(["nudge", "--to", "all", "--content", "ping"])
        .output()
        .expect("sm nudge executes");
    assert_success("sm nudge", &nudge_default);
    assert_total_line_count(&nudge_default, 1);

    let nudge_flag_scope = daemon
        .command()
        .current_dir(&alpha_dir)
        .args([
            "nudge",
            "--to",
            "role:engineer",
            "--namespace",
            "beta",
            "--content",
            "ping",
        ])
        .output()
        .expect("sm nudge --namespace executes");
    assert_success("sm nudge --namespace beta", &nudge_flag_scope);
    assert_contains_only(&nudge_flag_scope, &beta_id, &alpha_id);

    let nudge_all = daemon
        .command()
        .current_dir(&alpha_dir)
        .args(["nudge", "--to", "all", "--content", "ping", "-A"])
        .output()
        .expect("sm nudge -A executes");
    assert_success("sm nudge -A", &nudge_all);
    assert_total_line_count(&nudge_all, 2);

    let labeled = daemon
        .command()
        .current_dir(&alpha_dir)
        .args(["label", "all", "scope=alpha"])
        .output()
        .expect("sm label executes");
    assert_success("sm label", &labeled);
    assert_contains_only(&labeled, &alpha_id, &beta_id);

    let deleted = daemon
        .command()
        .current_dir(&alpha_dir)
        .args(["delete", "agent", "all", "--namespace", "beta"])
        .output()
        .expect("sm delete executes");
    assert_success("sm delete --namespace beta", &deleted);
    assert_contains_only(&deleted, &beta_id, &alpha_id);
}

#[test]
fn legacy_workspace_selector_is_rejected_by_cli() {
    let daemon = common::DaemonFixture::start();

    let selected = daemon
        .command()
        .args(["get", "agents", "--selector", "workspace:test", "-A"])
        .output()
        .expect("sm get agents executes");

    assert!(!selected.status.success());
    assert!(stderr(&selected).contains("unsupported selector"));
}

fn init_namespace(daemon: &common::DaemonFixture, namespace: &str, dir: &Path) {
    let output = daemon
        .command()
        .args([
            "init",
            "namespace",
            namespace,
            "--dir",
            &dir.display().to_string(),
        ])
        .output()
        .expect("sm init namespace executes");
    assert_success("sm init namespace", &output);
}

fn run_session(daemon: &common::DaemonFixture, namespace: &str, dir: &Path) -> String {
    let output = daemon
        .command()
        .args([
            "run",
            "claude",
            "--role",
            "engineer",
            "--dir",
            &dir.display().to_string(),
            "--namespace",
            namespace,
            "--detach",
        ])
        .output()
        .expect("sm run executes");
    assert_success("sm run", &output);
    first_field(&output.stdout)
}

fn assert_success(command: &str, output: &std::process::Output) {
    assert!(
        output.status.success(),
        "{command} failed\nstdout:\n{}\nstderr:\n{}",
        stdout(output),
        stderr(output)
    );
}

fn assert_contains_only(output: &std::process::Output, present: &str, absent: &str) {
    assert_contains(output, present);
    assert!(
        !stdout(output).contains(absent),
        "expected output to omit {absent}\nstdout:\n{}",
        stdout(output)
    );
}

fn assert_contains(output: &std::process::Output, needle: &str) {
    assert!(
        stdout(output).contains(needle),
        "expected output to contain {needle}\nstdout:\n{}",
        stdout(output)
    );
}

fn assert_line_count(bytes: &[u8], expected: usize) {
    let lines = String::from_utf8_lossy(bytes)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    assert_eq!(lines, expected);
}

fn assert_total_line_count(output: &std::process::Output, expected: usize) {
    let lines = [output.stdout.as_slice(), output.stderr.as_slice()]
        .into_iter()
        .map(nonempty_line_count)
        .sum::<usize>();
    assert_eq!(
        lines,
        expected,
        "stdout:\n{}\nstderr:\n{}",
        stdout(output),
        stderr(output)
    );
}

fn nonempty_line_count(bytes: &[u8]) -> usize {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

fn first_field(stdout: &[u8]) -> String {
    String::from_utf8_lossy(stdout)
        .split_whitespace()
        .next()
        .expect("stdout has first field")
        .to_string()
}

fn canonical(path: &Path) -> String {
    std::fs::canonicalize(path)
        .expect("path canonicalizes")
        .display()
        .to_string()
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}
