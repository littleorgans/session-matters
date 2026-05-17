#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

pub struct DaemonFixture {
    pub dir: tempfile::TempDir,
    child: Child,
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
        let mut command = Command::new(sm_bin());
        command
            .arg("__smd")
            .env("SM_HOME", dir.path())
            .env("HOME", dir.path());
        if let Some(prefix) = path_prefix {
            command.env("PATH", path_with_prefix(prefix));
        }
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("daemon starts");
        wait_for_socket(dir.path(), &mut child);
        Self { dir, child }
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

fn path_with_prefix(prefix: &Path) -> std::ffi::OsString {
    let paths = std::iter::once(prefix.to_path_buf()).chain(
        std::env::var_os("PATH")
            .into_iter()
            .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>()),
    );
    std::env::join_paths(paths).expect("PATH can be joined")
}
