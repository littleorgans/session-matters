use std::fs;
use std::path::PathBuf;

#[test]
fn root_version_flag_prints_session_matters_package_version() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_sm"))
        .arg("--version")
        .output()
        .expect("sm --version");

    assert!(output.status.success(), "sm --version failed: {output:?}");
    assert!(output.stderr.is_empty(), "stderr was not empty: {output:?}");

    let stdout = String::from_utf8(output.stdout).expect("version output utf8");
    let expected = format!("session-matters {}\n", env!("CARGO_PKG_VERSION"));
    assert_eq!(stdout, expected);
}

#[test]
fn install_recipe_switches_to_release_install() {
    let justfile = read_workspace_justfile();
    assert!(
        justfile.contains("\ninstall: install-release\n"),
        "just install must reinstall the release binary"
    );
}

#[test]
fn install_helper_prints_installed_binary_version() {
    let justfile = read_workspace_justfile();
    assert!(
        justfile.contains("\"$dest\" --version"),
        "install helper must print the version from the installed binary"
    );
}

fn read_workspace_justfile() -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path.push("justfile");
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}
