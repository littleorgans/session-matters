pub fn is_agent_config_path_like(value: &str) -> bool {
    value.contains(std::path::MAIN_SEPARATOR) || value.starts_with('~') || value.starts_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_toml_filename_is_not_path_like() {
        assert!(!is_agent_config_path_like("tools.toml"));
    }

    #[test]
    fn relative_dot_path_is_path_like() {
        assert!(is_agent_config_path_like("./tools.toml"));
    }

    #[test]
    fn separator_relative_path_is_path_like() {
        let value = format!("configs{}tools.toml", std::path::MAIN_SEPARATOR);

        assert!(is_agent_config_path_like(&value));
    }

    #[test]
    fn absolute_path_is_path_like() {
        let value = format!(
            "{}abs{}x.toml",
            std::path::MAIN_SEPARATOR,
            std::path::MAIN_SEPARATOR
        );

        assert!(is_agent_config_path_like(&value));
    }

    #[test]
    fn home_relative_path_is_path_like() {
        assert!(is_agent_config_path_like("~/x.toml"));
    }

    #[test]
    fn bare_name_is_not_path_like() {
        assert!(!is_agent_config_path_like("demo"));
    }
}
