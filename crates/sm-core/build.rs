use std::fs;
use std::path::Path;

#[path = "src/tool_sources.rs"]
mod tool_sources;

fn main() {
    println!("cargo:rerun-if-changed=../../tools");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set");
    let repo_root = Path::new(&manifest_dir).join("../..");
    let tools_dir = repo_root.join("tools");
    let paths = tool_sources::ordered_tool_source_paths(&tools_dir)
        .expect("tool source paths can be discovered");
    for path in &paths {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let generated = tool_sources::render_tool_source_includes(&repo_root, &paths)
        .expect("tool source includes can be rendered");
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR is set");
    fs::write(Path::new(&out_dir).join("tool_sources.rs"), generated)
        .expect("generated tool sources can be written");
}
