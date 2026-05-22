use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[path = "src/tool_contracts.rs"]
mod tool_contracts;
#[path = "src/tool_docs.rs"]
mod tool_docs;
#[path = "src/tool_examples.rs"]
mod tool_examples;
#[path = "../sm-core/src/tool_sources.rs"]
mod tool_sources;

use tool_contracts::{ToolContract, ToolContractRegistry};
use tool_docs::{
    render_generated_instructions_rs, render_readme_md, render_server_instructions, render_skill_md,
};

fn main() {
    println!("cargo:rerun-if-changed=../../tools");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/tool_contracts.rs");
    println!("cargo:rerun-if-changed=src/tool_docs.rs");
    println!("cargo:rerun-if-changed=src/tool_examples.rs");
    println!("cargo:rerun-if-changed=../sm-core/src/tool_sources.rs");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set");
    let tools_dir = Path::new(&manifest_dir).join("../../tools");
    let tool_paths =
        tool_sources::ordered_tool_source_paths(&tools_dir).expect("tool sources are discoverable");
    for path in &tool_paths {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let content = tool_sources::read_tool_sources(&tool_paths).expect("tool sources are readable");
    let registry = ToolContractRegistry::from_toml_str(&content).expect("tools/*.toml parses");

    write_schema_outputs(&manifest_dir, registry.tools());
    write_docs_outputs(&manifest_dir, &registry);
    emit_cli_version();
}

fn emit_cli_version() {
    emit_git_rerun_directives();
    println!("cargo:rerun-if-env-changed=SM_GIT_SHA");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    println!("cargo:rerun-if-env-changed=SM_VERSION_INCLUDE_GIT_SHA");

    let package_version = std::env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION set");
    let version = match (include_git_sha(), build_git_sha()) {
        (true, Some(sha)) => format!("{package_version}+{sha}"),
        _ => package_version,
    };
    println!("cargo:rustc-env=SM_CLI_VERSION={version}");
}

fn emit_git_rerun_directives() {
    let git_path = workspace_git_path();
    println!("cargo:rerun-if-changed={}", git_path.display());

    let Some(git_dir) = resolve_git_dir() else {
        return;
    };

    let head_path = git_dir.join("HEAD");
    println!("cargo:rerun-if-changed={}", head_path.display());

    let Ok(head) = fs::read_to_string(&head_path) else {
        return;
    };
    if let Some(ref_path) = head.trim().strip_prefix("ref: ") {
        println!(
            "cargo:rerun-if-changed={}",
            git_dir.join(ref_path).display()
        );
        if let Some(common_dir) = resolve_common_git_dir(&git_dir) {
            println!(
                "cargo:rerun-if-changed={}",
                common_dir.join(ref_path).display()
            );
            println!(
                "cargo:rerun-if-changed={}",
                common_dir.join("packed-refs").display()
            );
        }
    }
}

fn include_git_sha() -> bool {
    matches!(
        std::env::var("SM_VERSION_INCLUDE_GIT_SHA").as_deref(),
        Ok("1") | Ok("true")
    )
}

fn build_git_sha() -> Option<String> {
    std::env::var("SM_GIT_SHA")
        .ok()
        .and_then(short_sha)
        .or_else(|| std::env::var("GITHUB_SHA").ok().and_then(short_sha))
        .or_else(git_head_sha)
}

fn git_head_sha() -> Option<String> {
    let git_dir = resolve_git_dir()?;
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let trimmed = head.trim();
    if let Some(ref_path) = trimmed.strip_prefix("ref: ") {
        for dir in git_lookup_dirs(&git_dir) {
            let ref_file = dir.join(ref_path);
            if let Ok(sha) = fs::read_to_string(&ref_file) {
                return short_sha(sha.trim().to_string());
            }
        }
        for dir in git_lookup_dirs(&git_dir) {
            if let Ok(packed) = fs::read_to_string(dir.join("packed-refs")) {
                for line in packed.lines() {
                    if let Some((sha, name)) = line.split_once(' ')
                        && name == ref_path
                    {
                        return short_sha(sha.to_string());
                    }
                }
            }
        }
        None
    } else {
        short_sha(trimmed.to_string())
    }
}

fn workspace_git_path() -> PathBuf {
    PathBuf::from("../../.git")
}

fn resolve_git_dir() -> Option<PathBuf> {
    let git_path = workspace_git_path();
    if git_path.is_dir() {
        return Some(git_path);
    }

    let git_file = fs::read_to_string(&git_path).ok()?;
    let git_dir = git_file.trim().strip_prefix("gitdir: ")?;
    let git_dir = PathBuf::from(git_dir);
    if git_dir.is_absolute() {
        Some(git_dir)
    } else {
        Some(
            git_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(git_dir),
        )
    }
}

fn resolve_common_git_dir(git_dir: &Path) -> Option<PathBuf> {
    let common_dir = fs::read_to_string(git_dir.join("commondir")).ok()?;
    let common_dir = PathBuf::from(common_dir.trim());
    if common_dir.is_absolute() {
        Some(common_dir)
    } else {
        Some(git_dir.join(common_dir))
    }
}

fn git_lookup_dirs(git_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![git_dir.to_path_buf()];
    if let Some(common_dir) = resolve_common_git_dir(git_dir)
        && common_dir != git_dir
    {
        dirs.push(common_dir);
    }
    dirs
}

fn short_sha(sha: String) -> Option<String> {
    let trimmed = sha.trim();
    if trimmed.len() < 7 {
        return None;
    }
    Some(trimmed[..7].to_string())
}

fn write_schema_outputs(manifest_dir: &str, tools: &[ToolContract]) {
    let (schema_rs, schema_files) = generate_mcp_schema(tools);
    write_if_changed(
        &Path::new(manifest_dir).join("src/mcp/generated_schema.rs"),
        &schema_rs,
    );

    let schema_dir = Path::new(manifest_dir).join("src/mcp/generated_schema");
    fs::create_dir_all(&schema_dir).expect("generated schema dir can be created");
    let mut expected = HashSet::new();
    for (file_name, content) in &schema_files {
        expected.insert(file_name.as_str());
        write_if_changed(&schema_dir.join(file_name), content);
    }
    remove_stale_generated_files(&schema_dir, &expected);
}

fn write_docs_outputs(manifest_dir: &str, registry: &ToolContractRegistry) {
    let instructions = render_server_instructions(registry.skill(), registry.tools());
    let instructions_rs = render_generated_instructions_rs(&instructions);
    write_if_changed(
        &Path::new(manifest_dir).join("src/mcp/generated_instructions.rs"),
        &instructions_rs,
    );

    write_if_changed(
        &Path::new(manifest_dir).join("src/cli/generated_help.rs"),
        &generate_cli_help(registry),
    );

    let templates_dir = Path::new(manifest_dir).join("templates");
    fs::create_dir_all(&templates_dir).expect("templates dir can be created");
    write_if_changed(
        &templates_dir.join("SKILL.md"),
        &render_skill_md(registry.skill(), registry.tools()),
    );
    write_if_changed(
        &Path::new(manifest_dir).join("../../README.md"),
        &render_readme_md(registry.skill(), registry.tools()),
    );
}

fn generate_mcp_schema(tools: &[ToolContract]) -> (String, Vec<(String, String)>) {
    let mut include_lines = Vec::new();
    let mut schema_files = Vec::new();
    for tool in tools {
        let file_name = tool.artifacts.mcp_schema_file.clone();
        let tool_entry = tool.tool_entry_value();
        let json = serde_json::to_string_pretty(&tool_entry).expect("tool schema serializes");
        include_lines.push(format!(
            "        serde_json::from_str(include_str!(\"generated_schema/{file_name}\"))\n            .expect(\"generated schema for {} is valid JSON\"),",
            tool.name
        ));
        schema_files.push((file_name, format!("{json}\n")));
    }

    let mut schema_rs = String::new();
    schema_rs.push_str("// AUTO-GENERATED by build.rs from tools/*.toml - do not edit\n");
    schema_rs.push_str("#![allow(clippy::all)]\n\n");
    schema_rs.push_str("pub fn generated_tool_list() -> serde_json::Value {\n");
    schema_rs.push_str("    let tools: Vec<serde_json::Value> = vec![\n");
    for line in include_lines {
        schema_rs.push_str(&line);
        schema_rs.push('\n');
    }
    schema_rs.push_str("    ];\n");
    schema_rs.push_str("    serde_json::json!({ \"tools\": tools })\n");
    schema_rs.push_str("}\n");
    (schema_rs, schema_files)
}

fn generate_cli_help(registry: &ToolContractRegistry) -> String {
    let mut lines = vec![
        "// AUTO-GENERATED by build.rs from tools/*.toml - do not edit".to_string(),
        "#![allow(clippy::all)]".to_string(),
    ];
    for tool in registry.tools() {
        if !tool.artifacts.render_cli_help {
            continue;
        }
        let prefix = &tool.artifacts.cli_help_prefix;
        lines.push("#[rustfmt::skip]".to_string());
        lines.push(format!(
            "pub const {prefix}_ABOUT: &str = \"{}\";",
            rust_escape(&tool.cli.about)
        ));
        for param in &tool.params {
            if let Some(help) = &param.cli_help {
                let help = if param.selector {
                    format!("{help}\n\n{}", render_selector_help(registry))
                } else {
                    help.clone()
                };
                lines.push("#[rustfmt::skip]".to_string());
                lines.push(format!(
                    "pub const {prefix}_{}_HELP: &str = \"{}\";",
                    tool_contracts::rust_const_name(&param.name),
                    rust_escape(&help)
                ));
            }
        }
        lines.push(String::new());
    }
    lines.join("\n")
}

fn render_selector_help(registry: &ToolContractRegistry) -> String {
    let grammar = registry
        .shared()
        .selector_grammar
        .as_ref()
        .expect("shared.selector_grammar exists for selector CLI params");
    let mut lines = Vec::new();
    lines.push("Grammar:".to_string());
    lines.extend(grammar.forms.iter().map(|form| format!("  {form}")));
    lines.push("Examples:".to_string());
    lines.extend(
        grammar
            .examples
            .iter()
            .map(|example| format!("  {example}")),
    );
    lines.join("\n")
}

fn write_if_changed(path: &Path, content: &str) {
    if let Ok(existing) = fs::read_to_string(path)
        && existing == content
    {
        return;
    }
    fs::write(path, content).unwrap_or_else(|error| {
        panic!("failed to write {}: {error}", path.display());
    });
}

fn remove_stale_generated_files(dir: &Path, expected: &HashSet<&str>) {
    for entry in fs::read_dir(dir).expect("generated schema dir can be read") {
        let path = entry.expect("generated schema entry can be read").path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if path.extension().and_then(|ext| ext.to_str()) == Some("json")
            && !expected.contains(file_name)
        {
            fs::remove_file(&path).unwrap_or_else(|error| {
                panic!("failed to remove {}: {error}", path.display());
            });
        }
    }
}

fn rust_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
