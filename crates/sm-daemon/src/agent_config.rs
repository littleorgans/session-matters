use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use sm_driver::LaunchEnv;
use toml::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAgentConfig {
    pub requested: String,
    pub path: PathBuf,
    pub env: Vec<LaunchEnv>,
}

pub fn resolve_agent_config(requested: Option<&str>) -> Result<Option<ResolvedAgentConfig>> {
    let Some(requested) = requested else {
        return Ok(None);
    };
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("HOME is required for agent config resolution"))?;
    resolve_agent_config_with_home(requested, &home).map(Some)
}

fn resolve_agent_config_with_home(requested: &str, home: &Path) -> Result<ResolvedAgentConfig> {
    let path = agent_config_path(requested, home);
    if !path.is_file() {
        bail!(
            "agent config not found: {requested} (looked for {})",
            path.display()
        );
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read agent config {}", path.display()))?;
    let value = content
        .parse::<Value>()
        .with_context(|| format!("failed to parse agent config {}", path.display()))?;
    let env = agent_env(&value)?;

    Ok(ResolvedAgentConfig {
        requested: requested.to_string(),
        path,
        env,
    })
}

fn agent_config_path(requested: &str, home: &Path) -> PathBuf {
    if is_path_like(requested) {
        return expand_home(requested, home);
    }
    home.join(".agm").join(requested).join("agent.toml")
}

fn is_path_like(value: &str) -> bool {
    value.contains(std::path::MAIN_SEPARATOR)
        || value.starts_with('~')
        || value.starts_with('.')
        || value.ends_with(".toml")
}

fn expand_home(value: &str, home: &Path) -> PathBuf {
    if value == "~" {
        return home.to_path_buf();
    }
    if let Some(rest) = value.strip_prefix("~/") {
        return home.join(rest);
    }
    PathBuf::from(value)
}

fn agent_env(value: &Value) -> Result<Vec<LaunchEnv>> {
    let mut env = BTreeMap::new();
    if let Some(path) = value.get("claude_config_dir") {
        let path = path
            .as_str()
            .ok_or_else(|| anyhow!("agent config `claude_config_dir` must be a string"))?;
        env.insert("CLAUDE_CONFIG_DIR".to_string(), path.to_string());
    }
    if let Some(table) = value.get("env") {
        let table = table
            .as_table()
            .ok_or_else(|| anyhow!("agent config `env` must be a table"))?;
        for (key, value) in table {
            let value = value
                .as_str()
                .ok_or_else(|| anyhow!("agent config env `{key}` must be a string"))?;
            env.insert(key.to_string(), value.to_string());
        }
    }
    Ok(env
        .into_iter()
        .map(|(key, value)| LaunchEnv { key, value })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_named_agent_config_from_home_agm() {
        let dir = tempfile::tempdir().expect("tempdir creates");
        let config_dir = dir.path().join(".agm/demo-agent");
        fs::create_dir_all(&config_dir).expect("config dir creates");
        fs::write(
            config_dir.join("agent.toml"),
            "claude_config_dir = \"/tmp/claude\"\n[env]\nHELIOY_AGENT_NAME = \"demo\"\n",
        )
        .expect("config writes");

        let resolved =
            resolve_agent_config_with_home("demo-agent", dir.path()).expect("config resolves");

        assert_eq!(resolved.requested, "demo-agent");
        assert_eq!(
            resolved.env,
            vec![
                LaunchEnv {
                    key: "CLAUDE_CONFIG_DIR".to_string(),
                    value: "/tmp/claude".to_string(),
                },
                LaunchEnv {
                    key: "HELIOY_AGENT_NAME".to_string(),
                    value: "demo".to_string(),
                },
            ]
        );
    }

    #[test]
    fn resolves_explicit_agent_config_path() {
        let dir = tempfile::tempdir().expect("tempdir creates");
        let path = dir.path().join("agent.toml");
        fs::write(&path, "[env]\nHELIOY_AGENT_NAME = \"explicit\"\n").expect("config writes");

        let resolved =
            resolve_agent_config_with_home(path.to_str().expect("path is utf8"), dir.path())
                .expect("config resolves");

        assert_eq!(resolved.requested, path.to_string_lossy());
        assert_eq!(
            resolved.env,
            vec![LaunchEnv {
                key: "HELIOY_AGENT_NAME".to_string(),
                value: "explicit".to_string(),
            }]
        );
    }

    #[test]
    fn missing_agent_config_is_structured_error() {
        let dir = tempfile::tempdir().expect("tempdir creates");
        let error = resolve_agent_config_with_home("missing-agent", dir.path())
            .expect_err("missing config fails");

        assert!(error.to_string().contains("agent config not found"));
        assert!(error.to_string().contains("missing-agent"));
    }
}
