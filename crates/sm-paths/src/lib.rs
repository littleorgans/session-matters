use std::env;
use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};

#[cfg(test)]
#[path = "../../test_support.rs"]
mod test_support;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmPaths {
    pub dir: PathBuf,
    pub pidfile: PathBuf,
    pub database: PathBuf,
    pub log: PathBuf,
}

impl SmPaths {
    pub fn from_env() -> Result<Self, SmPathsError> {
        let dir = sm_home_dir()?;
        Ok(Self {
            pidfile: dir.join("sm.pid"),
            database: env_path("SM_DB_PATH").unwrap_or_else(|| dir.join("sm.db")),
            log: env_path("SM_LOG_PATH").unwrap_or_else(|| dir.join("smd.log")),
            dir,
        })
    }

    pub fn namespace_binding(&self) -> PathBuf {
        self.dir.join("namespace")
    }

    pub fn new(dir: PathBuf) -> Self {
        Self {
            pidfile: dir.join("sm.pid"),
            database: dir.join("sm.db"),
            log: dir.join("smd.log"),
            dir,
        }
    }
}

#[non_exhaustive]
#[derive(Clone, PartialEq, Eq)]
pub enum SmEndpoint {
    UnixSocket(PathBuf),
}

impl SmEndpoint {
    pub fn from_env() -> Result<Self, SmPathsError> {
        let dir = sm_home_dir()?;
        Ok(env_path("SM_SOCKET_PATH")
            .map_or_else(|| Self::UnixSocket(dir.join("sock")), Self::UnixSocket))
    }

    pub fn unix_socket(path: impl Into<PathBuf>) -> Self {
        Self::UnixSocket(path.into())
    }

    pub fn as_path(&self) -> &Path {
        match self {
            Self::UnixSocket(path) => path,
        }
    }

    pub fn exists(&self) -> bool {
        self.as_path().exists()
    }
}

impl fmt::Debug for SmEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnixSocket(path) => formatter.debug_tuple("UnixSocket").field(path).finish(),
        }
    }
}

impl fmt::Display for SmEndpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.as_path().display())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmPathsError {
    MissingHome,
}

impl fmt::Display for SmPathsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHome => write!(formatter, "home directory is not available"),
        }
    }
}

impl Error for SmPathsError {}

pub fn rtmd_socket_path() -> PathBuf {
    env_path("RTM_SOCKET_PATH")
        .or_else(|| env_path("XDG_RUNTIME_DIR").map(|dir| dir.join("rtm").join("sock")))
        .or_else(|| env_path("HOME").map(|home| home.join(".rtm").join("sock")))
        .unwrap_or_else(|| PathBuf::from(".rtm").join("sock"))
}

fn sm_home_dir() -> Result<PathBuf, SmPathsError> {
    env_path("SM_HOME")
        .or_else(|| env_path("HOME").map(|home| home.join(".sm")))
        .ok_or(SmPathsError::MissingHome)
}

fn env_path(name: &str) -> Option<PathBuf> {
    non_empty_env(name).map(PathBuf::from)
}

fn non_empty_env(name: &str) -> Option<OsString> {
    env::var_os(name).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::OrPanic as _;
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    #[test]
    fn sm_paths_preserve_home_defaults() {
        with_env(
            &[
                ("SM_HOME", Some("/tmp/sm-home")),
                ("SM_DB_PATH", None),
                ("SM_LOG_PATH", None),
            ],
            || {
                let paths = SmPaths::from_env().or_panic("paths resolve");
                assert_eq!(paths.dir, PathBuf::from("/tmp/sm-home"));
                assert_eq!(paths.pidfile, PathBuf::from("/tmp/sm-home/sm.pid"));
                assert_eq!(paths.database, PathBuf::from("/tmp/sm-home/sm.db"));
                assert_eq!(paths.log, PathBuf::from("/tmp/sm-home/smd.log"));
                assert_eq!(
                    paths.namespace_binding(),
                    PathBuf::from("/tmp/sm-home/namespace")
                );
            },
        );
    }

    #[test]
    fn sm_paths_use_database_and_log_overrides() {
        with_env(
            &[
                ("SM_HOME", Some("/tmp/sm-home")),
                ("SM_DB_PATH", Some("/tmp/custom.db")),
                ("SM_LOG_PATH", Some("/tmp/custom.log")),
            ],
            || {
                let paths = SmPaths::from_env().or_panic("paths resolve");
                assert_eq!(paths.database, PathBuf::from("/tmp/custom.db"));
                assert_eq!(paths.log, PathBuf::from("/tmp/custom.log"));
            },
        );
    }

    #[test]
    fn sm_endpoint_uses_socket_override() {
        with_env(
            &[
                ("SM_HOME", Some("/tmp/sm-home")),
                ("SM_SOCKET_PATH", Some("/tmp/sm.sock")),
            ],
            || {
                let endpoint = SmEndpoint::from_env().or_panic("endpoint resolves");
                assert_eq!(endpoint.as_path(), Path::new("/tmp/sm.sock"));
                assert_eq!(endpoint.to_string(), "/tmp/sm.sock");
                assert!(format!("{endpoint:?}").starts_with("UnixSocket"));
            },
        );
    }

    #[test]
    fn sm_endpoint_uses_home_socket_default() {
        with_env(
            &[
                ("SM_HOME", None),
                ("SM_SOCKET_PATH", None),
                ("HOME", Some("/tmp/home")),
            ],
            || {
                let endpoint = SmEndpoint::from_env().or_panic("endpoint resolves");
                assert_eq!(endpoint.as_path(), Path::new("/tmp/home/.sm/sock"));
            },
        );
    }

    #[test]
    fn empty_env_values_are_ignored() {
        with_env(
            &[
                ("SM_HOME", Some("")),
                ("SM_SOCKET_PATH", Some("")),
                ("SM_DB_PATH", Some("")),
                ("SM_LOG_PATH", Some("")),
                ("HOME", Some("/tmp/home")),
            ],
            || {
                let paths = SmPaths::from_env().or_panic("paths resolve");
                let endpoint = SmEndpoint::from_env().or_panic("endpoint resolves");
                assert_eq!(paths.dir, PathBuf::from("/tmp/home/.sm"));
                assert_eq!(paths.database, PathBuf::from("/tmp/home/.sm/sm.db"));
                assert_eq!(paths.log, PathBuf::from("/tmp/home/.sm/smd.log"));
                assert_eq!(endpoint.as_path(), Path::new("/tmp/home/.sm/sock"));
            },
        );
    }

    #[test]
    fn missing_home_errors() {
        with_env(&[("SM_HOME", None), ("HOME", None)], || {
            assert_eq!(SmPaths::from_env(), Err(SmPathsError::MissingHome));
            assert_eq!(SmEndpoint::from_env(), Err(SmPathsError::MissingHome));
        });
    }

    #[test]
    fn rtmd_socket_path_prefers_explicit_env() {
        with_env(
            &[
                ("RTM_SOCKET_PATH", Some("/tmp/rtm.sock")),
                ("XDG_RUNTIME_DIR", Some("/run/user/501")),
                ("HOME", Some("/tmp/home")),
            ],
            || assert_eq!(rtmd_socket_path(), PathBuf::from("/tmp/rtm.sock")),
        );
    }

    #[test]
    fn rtmd_socket_path_uses_xdg_runtime_dir_before_home() {
        with_env(
            &[
                ("RTM_SOCKET_PATH", None),
                ("XDG_RUNTIME_DIR", Some("/run/user/501")),
                ("HOME", Some("/tmp/home")),
            ],
            || assert_eq!(rtmd_socket_path(), PathBuf::from("/run/user/501/rtm/sock")),
        );
    }

    #[test]
    fn rtmd_socket_path_falls_back_to_home() {
        with_env(
            &[
                ("RTM_SOCKET_PATH", None),
                ("XDG_RUNTIME_DIR", None),
                ("HOME", Some("/tmp/home")),
            ],
            || assert_eq!(rtmd_socket_path(), PathBuf::from("/tmp/home/.rtm/sock")),
        );
    }

    // Rust 2024 marks process env mutation unsafe. The lock keeps these env
    // changes scoped and serial for path resolution tests.
    #[allow(unsafe_code)]
    fn with_env<T>(vars: &[(&str, Option<&str>)], test: impl FnOnce() -> T) -> T {
        let _guard = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .or_panic("env lock is not poisoned");
        let previous: Vec<_> = vars
            .iter()
            .map(|(name, _)| (*name, env::var_os(name)))
            .collect();
        for (name, value) in vars {
            match value {
                Some(value) => unsafe { env::set_var(name, value) },
                None => unsafe { env::remove_var(name) },
            }
        }
        let result = test();
        for (name, value) in previous {
            match value {
                Some(value) => unsafe { env::set_var(name, value) },
                None => unsafe { env::remove_var(name) },
            }
        }
        result
    }
}
