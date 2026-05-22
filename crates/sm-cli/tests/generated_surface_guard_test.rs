use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn generated_help_surface_has_no_agent_help_constants() {
    for path in tool_source_paths() {
        assert_absent(&path, "AGENT_");
    }
    assert_absent(
        &repo_root().join("crates/sm-cli/src/cli/generated_help.rs"),
        "pub const AGENT_",
    );
}

#[test]
fn generated_docs_do_not_reference_removed_cli_forms() {
    let forbidden = [
        "sm get agent",
        "sm get agents",
        "sm delete agent",
        "sm init namespace",
        ".sm/namespace",
    ];
    for path in removed_surface_guard_paths() {
        for token in forbidden {
            assert_absent(&path, token);
        }
    }
}

#[test]
fn selector_help_sources_match_cli_shape_matrix() {
    let cases = [
        (
            "tools/session.toml",
            "[tools.session_list]",
            "cli_flag        = \"--selector\"",
        ),
        (
            "tools/session.toml",
            "[tools.session_delete]",
            "cli_flag        = \"selector\"",
        ),
        (
            "tools/label.toml",
            "[tools.session_label]",
            "cli_flag        = \"selector\"",
        ),
        (
            "tools/logs.toml",
            "[tools.logs]",
            "cli_flag        = \"selector\"",
        ),
        (
            "tools/wait.toml",
            "[tools.wait]",
            "cli_flag        = \"selector\"",
        ),
        (
            "tools/capture.toml",
            "[tools.session_capture]",
            "name            = \"id\"",
        ),
        (
            "tools/mail.toml",
            "[tools.mail_send]",
            "cli_flag        = \"--to\"",
        ),
        (
            "tools/mail.toml",
            "[tools.mail_read]",
            "cli_flag        = \"--selector\"",
        ),
        (
            "tools/mail.toml",
            "[tools.mail_check]",
            "cli_flag        = \"--selector\"",
        ),
        (
            "tools/mail.toml",
            "[tools.mail_stop_check]",
            "cli_flag        = \"--selector\"",
        ),
        (
            "tools/nudge.toml",
            "[tools.nudge]",
            "cli_flag        = \"--to\"",
        ),
    ];
    for (path, table, flag) in cases {
        let content = read_repo_file(path);
        let table_body = content
            .split(table)
            .nth(1)
            .unwrap_or_else(|| panic!("{path} missing table {table}"));
        assert!(
            table_body.contains(flag),
            "{path} {table} missing expected selector shape {flag}"
        );
    }
}

fn removed_surface_guard_paths() -> Vec<PathBuf> {
    let mut paths = repo_paths([
        "crates/sm-cli/src/tool_docs.rs",
        "crates/sm-cli/templates/SKILL.md",
        "README.md",
        "crates/sm-cli/src/mcp/generated_instructions.rs",
    ]);
    paths.extend(generated_schema_paths());
    paths
}

fn tool_source_paths() -> Vec<PathBuf> {
    let tools_dir = repo_root().join("tools");
    fs::read_dir(&tools_dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", tools_dir.display()))
        .map(|entry| entry.expect("tool source entry reads").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .collect()
}

fn generated_schema_paths() -> Vec<PathBuf> {
    let schema_dir = repo_root().join("crates/sm-cli/src/mcp/generated_schema");
    fs::read_dir(&schema_dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", schema_dir.display()))
        .map(|entry| entry.expect("generated schema entry reads").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect()
}

fn repo_paths<const N: usize>(paths: [&str; N]) -> Vec<PathBuf> {
    paths
        .into_iter()
        .map(|path| repo_root().join(path))
        .collect()
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("sm-cli manifest is under crates/sm-cli")
        .to_path_buf()
}

fn assert_absent(path: &Path, token: &str) {
    let content = read_file(path);
    assert!(
        !content.contains(token),
        "{} contains forbidden token {token:?}",
        path.display()
    );
}

fn read_repo_file(path: &str) -> String {
    read_file(&repo_root().join(path))
}

fn read_file(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}
