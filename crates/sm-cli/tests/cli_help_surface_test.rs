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
fn retained_leaf_commands_print_help_on_bare_invocation() {
    for args in [
        ["label"].as_slice(),
        ["logs"].as_slice(),
        ["capture"].as_slice(),
        ["wait"].as_slice(),
        ["nudge"].as_slice(),
        ["run"].as_slice(),
    ] {
        let stdout = help(args);
        assert!(
            stdout.contains("Usage:"),
            "sm {} did not print help\n{stdout}",
            args.join(" ")
        );
    }
}

#[test]
fn label_help_describes_positionals_and_selector_grammar() {
    let stdout = help(&["label", "--help"]);

    for expected in [
        "<SELECTOR>",
        "Session selector to mutate.",
        "<MUTATION>",
        "Label mutation as key=value or key-.",
        "Grammar:",
        "all",
        "<uuid>",
        "id:<uuid>",
        "Examples:",
        "019e44f9-...",
        "role:engineer",
    ] {
        assert!(
            stdout.contains(expected),
            "label help missing {expected:?}\n{stdout}"
        );
    }
    assert!(
        !stdout.contains("Grammar: all,"),
        "selector grammar should render vertically\n{stdout}"
    );
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

#[test]
fn get_help_collapses_resources_to_singular_with_plural_aliases() {
    let get = help(&["get", "--help"]);
    assert!(get.contains("List session records, or get one session record by id."));
    assert!(get.contains("List namespace records, or get one namespace record by slug."));
    assert!(!get.contains("List session records known to the session-matters daemon."));
    assert!(!get.contains("List namespace records\n"));

    let session = help(&["get", "sessions", "--help"]);
    assert!(session.contains("Optional session id to load instead of listing."));
    assert!(session.contains("--selector"));

    let namespace = help(&["get", "namespaces", "--help"]);
    assert!(namespace.contains("Optional namespace slug to load instead of listing."));
    assert!(!namespace.contains("--selector"));
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
