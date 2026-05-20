#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

pub struct DaemonFixture {
    pub dir: tempfile::TempDir,
    child: Child,
    rtmd: Child,
    rtm: PathBuf,
    rtm_socket: PathBuf,
}

impl DaemonFixture {
    pub fn start() -> Self {
        Self::start_with_path_prefix(None)
    }

    pub fn start_with_runtime_path(path_prefix: &Path) -> Self {
        Self::start_with_path_prefix(Some(path_prefix))
    }

    fn start_with_path_prefix(path_prefix: Option<&Path>) -> Self {
        let dir = tempfile::tempdir().expect("tempdir creates");
        let rtm_socket = dir.path().join("rtm.sock");
        let rtm = rtm_bin();
        let mut rtmd = Command::new(&rtm)
            .arg("daemon")
            .arg("start")
            .env("RTM_SOCKET_PATH", &rtm_socket)
            .env("RTM_DB_PATH", dir.path().join("rtm.sqlite"))
            .env("RTM_HOME", dir.path().join("rtm-home"))
            .env("PATH", test_path(path_prefix))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("rtmd starts");
        wait_for_path_socket(&rtm_socket, &mut rtmd);

        let mut command = Command::new(sm_bin());
        command
            .arg("__smd")
            .env("SM_HOME", dir.path())
            .env("HOME", dir.path())
            .env("RTM_SOCKET_PATH", &rtm_socket)
            .env("PATH", test_path(path_prefix));
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("daemon starts");
        wait_for_socket(dir.path(), &mut child);
        Self {
            dir,
            child,
            rtmd,
            rtm,
            rtm_socket,
        }
    }

    pub fn spawn_mcp(&self) -> McpFixture {
        let child = self
            .command()
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("sm mcp starts");
        McpFixture {
            child,
            stdin: None,
            stdout: None,
        }
        .with_pipes()
    }

    pub fn audit_path(&self) -> PathBuf {
        self.dir.path().join(".im").join("audit.sqlite")
    }

    pub fn socket_path(&self) -> PathBuf {
        self.dir.path().join("sock")
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new(sm_bin());
        command
            .env("SM_HOME", self.dir.path())
            .env("HOME", self.dir.path());
        command
    }

    fn stop(&mut self) {
        let _ = self
            .command()
            .args(["daemon", "stop"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = self.child.wait();
        let _ = Command::new(&self.rtm)
            .args(["daemon", "stop"])
            .env("RTM_SOCKET_PATH", &self.rtm_socket)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = self.rtmd.kill();
        let _ = self.rtmd.wait();
    }
}

impl Drop for DaemonFixture {
    fn drop(&mut self) {
        self.stop();
    }
}

pub struct McpFixture {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
}

impl McpFixture {
    fn with_pipes(mut self) -> Self {
        self.stdin = Some(self.child.stdin.take().expect("mcp stdin"));
        self.stdout = Some(BufReader::new(
            self.child.stdout.take().expect("mcp stdout"),
        ));
        self
    }

    pub fn send(&mut self, request: &Value) -> Value {
        let line = serde_json::to_string(request).expect("request serializes");
        let stdin = self.stdin.as_mut().expect("mcp stdin open");
        writeln!(stdin, "{line}").expect("request writes");
        stdin.flush().expect("request flushes");

        let mut response = String::new();
        self.stdout
            .as_mut()
            .expect("mcp stdout open")
            .read_line(&mut response)
            .expect("response reads");
        serde_json::from_str(&response).expect("response parses")
    }
}

impl Drop for McpFixture {
    fn drop(&mut self) {
        drop(self.stdin.take());
        let _ = self.child.wait();
    }
}

pub fn sm_bin() -> PathBuf {
    if let Some(path) = std::env::var_os("SM_BENCH_BIN") {
        return PathBuf::from(path);
    }
    assert_cmd::cargo::cargo_bin("sm")
}

fn wait_for_socket(dir: &Path, child: &mut Child) {
    let socket = dir.join("sock");
    wait_for_path_socket(&socket, child);
}

fn wait_for_path_socket(socket: &Path, child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if socket.exists() {
            return;
        }
        if let Some(exit) = child.try_wait().expect("daemon can be observed") {
            panic!("daemon exited before socket became ready: {exit}");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("daemon socket did not become ready");
}

pub fn fake_runtime_path(command: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("runtime path tempdir creates");
    let runtime = dir.path().join(command);
    std::fs::write(
        &runtime,
        "#!/bin/sh\ntrap 'exit 0' TERM INT\nwhile :; do sleep 60; done\n",
    )
    .expect("fake runtime writes");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(&runtime)
            .expect("fake runtime metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&runtime, permissions).expect("fake runtime is executable");
    }

    dir
}

fn rtm_bin() -> PathBuf {
    if let Some(path) = std::env::var_os("RTM_TEST_BIN") {
        return PathBuf::from(path);
    }
    let sibling = helioy_root().join("runtime-matters/target/debug/rtm");
    if sibling.exists() {
        return sibling;
    }
    PathBuf::from("rtm")
}

fn helioy_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(4)
        .expect("workspace has helioy root ancestor")
        .to_path_buf()
}

fn test_path(prefix: Option<&Path>) -> std::ffi::OsString {
    let prefixes = prefix.into_iter().map(Path::to_path_buf);
    let paths = prefixes.chain(
        std::env::var_os("PATH")
            .into_iter()
            .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>()),
    );
    std::env::join_paths(paths).expect("PATH can be joined")
}
