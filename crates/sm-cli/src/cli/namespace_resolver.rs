use std::path::{Path, PathBuf};
use std::str::FromStr;

use sm_core::{Namespace, NamespaceError};

const MARKER_PATH: &[&str] = &[".sm", "namespace"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceResolution {
    pub namespace: Namespace,
    pub canonical_dir: PathBuf,
    pub warnings: Vec<NamespaceResolutionWarning>,
}

impl NamespaceResolution {
    pub fn into_pair(self) -> (Namespace, PathBuf) {
        (self.namespace, self.canonical_dir)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamespaceResolutionWarning {
    MissingOrInvalidHome,
}

#[derive(Debug, thiserror::Error)]
pub enum NamespaceResolutionError {
    #[error("failed to read namespace marker {path}: {source}")]
    ReadMarker {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid namespace marker {path}: {source}")]
    InvalidMarker {
        path: PathBuf,
        #[source]
        source: NamespaceError,
    },
    #[error("failed to canonicalize namespace directory {path}: {source}")]
    CanonicalizeDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub fn resolve_namespace_dir(
    start_dir: impl AsRef<Path>,
    explicit_namespace: Option<Namespace>,
) -> Result<NamespaceResolution, NamespaceResolutionError> {
    resolve_namespace_dir_with_home(start_dir.as_ref(), explicit_namespace, env_home_dir())
}

fn resolve_namespace_dir_with_home(
    start_dir: &Path,
    explicit_namespace: Option<Namespace>,
    home_dir: Option<PathBuf>,
) -> Result<NamespaceResolution, NamespaceResolutionError> {
    let (home_boundary, warnings) = match home_dir {
        Some(home) if home.is_absolute() && home.is_dir() => (Some(home), Vec::new()),
        _ => (None, vec![NamespaceResolutionWarning::MissingOrInvalidHome]),
    };

    if let Some(namespace) = explicit_namespace {
        return Ok(NamespaceResolution {
            namespace,
            canonical_dir: canonical_dir(start_dir)?,
            warnings,
        });
    }

    let mut current = Some(start_dir);
    while let Some(dir) = current {
        let marker = marker_path(dir);
        let marker_exists =
            marker
                .try_exists()
                .map_err(|source| NamespaceResolutionError::ReadMarker {
                    path: marker.clone(),
                    source,
                })?;
        if marker_exists {
            let raw = std::fs::read_to_string(&marker).map_err(|source| {
                NamespaceResolutionError::ReadMarker {
                    path: marker.clone(),
                    source,
                }
            })?;
            let namespace = Namespace::from_str(raw.trim()).map_err(|source| {
                NamespaceResolutionError::InvalidMarker {
                    path: marker,
                    source,
                }
            })?;
            return Ok(NamespaceResolution {
                namespace,
                canonical_dir: canonical_dir(dir)?,
                warnings,
            });
        }
        if home_boundary.as_deref() == Some(dir) {
            break;
        }
        current = dir.parent();
    }

    Ok(NamespaceResolution {
        namespace: Namespace::default(),
        canonical_dir: canonical_dir(start_dir)?,
        warnings,
    })
}

fn env_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn marker_path(dir: &Path) -> PathBuf {
    MARKER_PATH
        .iter()
        .fold(dir.to_path_buf(), |path, segment| path.join(segment))
}

fn canonical_dir(dir: &Path) -> Result<PathBuf, NamespaceResolutionError> {
    std::fs::canonicalize(dir).map_err(|source| NamespaceResolutionError::CanonicalizeDir {
        path: dir.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_marker_at_start_dir() {
        let root = tempfile::tempdir().expect("tempdir");
        let start = root.path().join("project");
        write_marker(&start, "alpha");

        let resolved = resolve_with_home(&start, Some(root.path()));

        assert_eq!(resolved.namespace.as_str(), "alpha");
        assert_eq!(resolved.canonical_dir, canonical(&start));
    }

    #[test]
    fn resolves_marker_at_ancestor() {
        let root = tempfile::tempdir().expect("tempdir");
        let project = root.path().join("project");
        let nested = project.join("src/bin");
        std::fs::create_dir_all(&nested).expect("nested dirs");
        write_marker(&project, "ancestor");

        let resolved = resolve_with_home(&nested, Some(root.path()));

        assert_eq!(resolved.namespace.as_str(), "ancestor");
        assert_eq!(resolved.canonical_dir, canonical(&project));
    }

    #[test]
    fn falls_back_to_default_when_no_marker_found() {
        let root = tempfile::tempdir().expect("tempdir");
        let start = root.path().join("project");
        std::fs::create_dir_all(&start).expect("start dir");

        let resolved = resolve_with_home(&start, Some(root.path()));

        assert_eq!(resolved.namespace, Namespace::default());
        assert_eq!(resolved.canonical_dir, canonical(&start));
    }

    #[test]
    fn resolves_marker_at_home_boundary() {
        let home = tempfile::tempdir().expect("tempdir");
        let start = home.path().join("project");
        std::fs::create_dir_all(&start).expect("start dir");
        write_marker(home.path(), "home");

        let resolved = resolve_with_home(&start, Some(home.path()));

        assert_eq!(resolved.namespace.as_str(), "home");
        assert_eq!(resolved.canonical_dir, canonical(home.path()));
    }

    #[test]
    fn cwd_outside_home_walks_to_root() {
        let outside = tempfile::tempdir().expect("outside");
        let home = tempfile::tempdir().expect("home");
        let parent = outside.path().join("parent");
        let start = parent.join("child");
        std::fs::create_dir_all(&start).expect("start dir");
        write_marker(&parent, "outside");

        let resolved = resolve_with_home(&start, Some(home.path()));

        assert_eq!(resolved.namespace.as_str(), "outside");
        assert_eq!(resolved.canonical_dir, canonical(&parent));
    }

    #[test]
    fn symlink_walk_is_lexical_and_return_dir_is_canonical() {
        let root = tempfile::tempdir().expect("tempdir");
        let real = root.path().join("real");
        let link = root.path().join("link");
        let start = link.join("child");
        std::fs::create_dir_all(real.join("child")).expect("real child");
        symlink_dir(&real, &link);
        write_marker(&link, "linked");

        let resolved = resolve_with_home(&start, Some(root.path()));

        assert_eq!(resolved.namespace.as_str(), "linked");
        assert_eq!(resolved.canonical_dir, canonical(&real));
    }

    #[test]
    fn invalid_marker_content_fails_loudly() {
        let root = tempfile::tempdir().expect("tempdir");
        let start = root.path().join("project");
        write_marker(&start, "Alpha");

        let error = resolve_error(&start, Some(root.path()));

        assert!(matches!(
            error,
            NamespaceResolutionError::InvalidMarker { .. }
        ));
        assert!(error.to_string().contains("invalid namespace marker"));
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_marker_fails_loudly() {
        let root = tempfile::tempdir().expect("tempdir");
        let start = root.path().join("project");
        let marker = write_marker(&start, "alpha");
        remove_read_permissions(&marker);

        let error = resolve_error(&start, Some(root.path()));

        restore_read_permissions(&marker);
        assert!(matches!(error, NamespaceResolutionError::ReadMarker { .. }));
    }

    #[test]
    fn explicit_namespace_uses_start_dir_canonical_path() {
        let root = tempfile::tempdir().expect("tempdir");
        let start = root.path().join("project");
        write_marker(&start, "marker");

        let resolved = resolve_namespace_dir_with_home(
            &start,
            Some(Namespace::new("explicit").expect("namespace")),
            Some(root.path().to_path_buf()),
        )
        .expect("resolves");

        assert_eq!(resolved.namespace.as_str(), "explicit");
        assert_eq!(resolved.canonical_dir, canonical(&start));
    }

    #[test]
    fn missing_or_invalid_home_warns_and_falls_back_to_default() {
        let root = tempfile::tempdir().expect("tempdir");
        let start = root.path().join("project");
        std::fs::create_dir_all(&start).expect("start dir");

        let resolved =
            resolve_namespace_dir_with_home(&start, None, None).expect("resolves without home");

        assert_eq!(resolved.namespace, Namespace::default());
        assert_eq!(
            resolved.warnings,
            vec![NamespaceResolutionWarning::MissingOrInvalidHome]
        );
        assert_eq!(resolved.canonical_dir, canonical(&start));
    }

    #[test]
    fn nonexistent_home_warns_and_walks_to_root() {
        let root = tempfile::tempdir().expect("tempdir");
        let parent = root.path().join("parent");
        let start = parent.join("child");
        std::fs::create_dir_all(&start).expect("start dir");
        write_marker(&parent, "root-walk");

        let resolved =
            resolve_namespace_dir_with_home(&start, None, Some(root.path().join("missing-home")))
                .expect("resolves with invalid home");

        assert_eq!(resolved.namespace.as_str(), "root-walk");
        assert_eq!(resolved.canonical_dir, canonical(&parent));
        assert_eq!(
            resolved.warnings,
            vec![NamespaceResolutionWarning::MissingOrInvalidHome]
        );
    }

    fn resolve_with_home(start: &Path, home: Option<&Path>) -> NamespaceResolution {
        resolve_namespace_dir_with_home(start, None, home.map(Path::to_path_buf))
            .expect("namespace resolves")
    }

    fn resolve_error(start: &Path, home: Option<&Path>) -> NamespaceResolutionError {
        resolve_namespace_dir_with_home(start, None, home.map(Path::to_path_buf))
            .expect_err("namespace fails")
    }

    fn write_marker(dir: &Path, value: &str) -> PathBuf {
        let marker = marker_path(dir);
        std::fs::create_dir_all(marker.parent().expect("marker parent")).expect("marker dir");
        std::fs::write(&marker, value).expect("marker write");
        marker
    }

    fn canonical(path: &Path) -> PathBuf {
        std::fs::canonicalize(path).expect("canonical path")
    }

    #[cfg(unix)]
    fn symlink_dir(original: &Path, link: &Path) {
        std::os::unix::fs::symlink(original, link).expect("symlink dir");
    }

    #[cfg(windows)]
    fn symlink_dir(original: &Path, link: &Path) {
        std::os::windows::fs::symlink_dir(original, link).expect("symlink dir");
    }

    #[cfg(unix)]
    fn remove_read_permissions(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(path)
            .expect("marker metadata")
            .permissions();
        permissions.set_mode(0o000);
        std::fs::set_permissions(path, permissions).expect("remove read permissions");
    }

    #[cfg(unix)]
    fn restore_read_permissions(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(path)
            .expect("marker metadata")
            .permissions();
        permissions.set_mode(0o600);
        std::fs::set_permissions(path, permissions).expect("restore read permissions");
    }
}
