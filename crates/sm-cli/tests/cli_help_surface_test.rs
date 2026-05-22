use std::process::{Command, Output};

#[test]
fn top_level_help_describes_visible_commands() {
    let stdout = help(&["--help"]);
    for description in [
        "Manage the session-matters daemon",
        "Imperatively run a session through the session-matters daemon",
        "Create namespace and session records",
        "Manage session-matters user configuration",
        "Inspect sessions and namespaces",
        "Delete sessions and namespaces",
        "Report session-matters daemon health",
        "Send and read durable session mail",
        "Add or remove labels on selected sessions",
        "Bridge MCP stdio to the session-matters daemon",
    ] {
        assert!(
            stdout.contains(description),
            "top-level help missing {description:?}\n{stdout}"
        );
    }
}

#[test]
fn daemon_help_describes_subcommands() {
    let stdout = help(&["daemon", "--help"]);
    for description in [
        "Start the session-matters daemon",
        "Stop the session-matters daemon",
        "Show session-matters daemon status",
    ] {
        assert!(
            stdout.contains(description),
            "daemon help missing {description:?}\n{stdout}"
        );
    }
}

#[test]
fn run_help_describes_every_flag() {
    let stdout = help(&["run", "--help"]);
    for description in [
        "Runtime to launch",
        "Role label recorded on the session",
        "Filesystem directory used as the runtime's working directory",
        "Namespace slug for the session",
        "Session label as key=value",
        "Agent config name or explicit agent.toml path",
        "Runtime target",
        "Preempt an occupied tmux pane",
        "Return after creating the session instead of waiting on the runtime",
    ] {
        assert!(
            stdout.contains(description),
            "run help missing {description:?}\n{stdout}"
        );
    }
}

#[test]
fn create_and_delete_resource_help_uses_current_lifecycle_copy() {
    let create = help(&["create", "--help"]);
    assert!(create.contains("Create namespace and session records"));
    assert!(create.contains("Create a namespace before running sessions into it"));
    assert!(create.contains("Declaratively create a headless session record"));

    let run = help(&["run", "--help"]);
    assert!(run.contains("Imperatively run a session through the session-matters daemon"));

    let config = help(&["config", "--help"]);
    assert!(config.contains("Set the user namespace context used by CLI commands"));

    let delete = help(&["delete", "--help"]);
    assert!(delete.contains("Terminate daemon owned sessions by selector"));
    assert!(delete.contains("Delete a namespace, terminate its sessions"));
}

fn help(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_sm"))
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("sm {} executes: {error}", args.join(" ")));
    assert_success(&format!("sm {}", args.join(" ")), &output);
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn assert_success(command: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{command} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
