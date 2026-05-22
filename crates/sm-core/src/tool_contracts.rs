#![allow(dead_code)]

use std::collections::HashSet;
use std::sync::OnceLock;

use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::{Map, Value, json};

include!(concat!(env!("OUT_DIR"), "/tool_sources.rs"));

static REGISTRY: OnceLock<ToolContractRegistry> = OnceLock::new();

pub fn contract_registry() -> &'static ToolContractRegistry {
    REGISTRY.get_or_init(|| {
        let content = bundled_tool_sources();
        ToolContractRegistry::from_toml_str(&content)
            .unwrap_or_else(|error| panic!("tools/*.toml sources are valid: {error}"))
    })
}

fn bundled_tool_sources() -> String {
    let mut content = String::new();
    for (_, source) in TOOL_SOURCE_FILES {
        content.push_str(source);
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content.push('\n');
    }
    content
}

#[derive(Debug, Clone)]
pub struct ToolContractRegistry {
    shared: SharedContent,
    skill: Option<SkillConfig>,
    tools: Vec<ToolContract>,
}

impl ToolContractRegistry {
    pub fn from_toml_str(content: &str) -> Result<Self, String> {
        let parsed: RawToolsToml = toml::from_str(content)
            .map_err(|error| format!("failed to parse tools/*.toml: {error}"))?;
        let mut raw_tools = parsed.tools.into_iter().enumerate().collect::<Vec<_>>();
        raw_tools.sort_by(
            |(left_index, (left_name, _)), (right_index, (right_name, _))| {
                tool_contract_sort_key(left_name, *left_index)
                    .cmp(&tool_contract_sort_key(right_name, *right_index))
            },
        );

        let tools = raw_tools
            .into_iter()
            .map(|(_, tool)| tool)
            .map(|(name, raw)| {
                let aliases = raw.mcp_aliases.clone();
                let tool = ToolContract::from_raw(name, raw)?;
                let mut tools = Vec::with_capacity(aliases.len() + 1);
                tools.push(tool.clone());
                tools.extend(
                    aliases
                        .into_iter()
                        .map(|alias| tool.alias(alias))
                        .collect::<Result<Vec<_>, _>>()?,
                );
                Ok(tools)
            })
            .collect::<Result<Vec<_>, String>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        validate_unique_render_names(&tools)?;

        Ok(Self {
            shared: parsed.shared.unwrap_or_default(),
            skill: parsed.skill,
            tools,
        })
    }

    pub fn shared(&self) -> &SharedContent {
        &self.shared
    }

    pub fn skill(&self) -> Option<&SkillConfig> {
        self.skill.as_ref()
    }

    pub fn tools(&self) -> &[ToolContract] {
        &self.tools
    }

    pub fn tool_list_value(&self) -> Value {
        let tools = self
            .tools
            .iter()
            .map(|tool| tool.tool_entry_value(&self.shared))
            .collect::<Vec<_>>();
        json!({ "tools": tools })
    }
}

const TOOL_CONTRACT_ORDER: &[&str] = &[
    "session_run",
    "session_list",
    "session_get",
    "namespace_list",
    "namespace_get",
    "session_capture",
    "session_delete",
    "session_label",
    "mail_send",
    "mail_read",
    "mail_check",
    "mail_stop_check",
    "nudge",
    "logs",
    "wait",
    "doctor",
];

fn tool_contract_sort_key(name: &str, original_index: usize) -> (usize, usize) {
    let index = TOOL_CONTRACT_ORDER
        .iter()
        .position(|known| *known == name)
        .unwrap_or(TOOL_CONTRACT_ORDER.len());
    (index, original_index)
}

fn validate_unique_render_names(tools: &[ToolContract]) -> Result<(), String> {
    let mut tool_names = HashSet::new();
    let mut const_names = HashSet::new();
    for tool in tools {
        if !tool_names.insert(tool.name.as_str()) {
            return Err(format!("duplicate MCP tool name {}", tool.name));
        }
        if !tool.artifacts.render_cli_help {
            continue;
        }
        let prefix = &tool.artifacts.cli_help_prefix;
        let about_const = format!("{prefix}_ABOUT");
        if !const_names.insert(about_const.clone()) {
            return Err(format!("duplicate CLI help constant {about_const}"));
        }
        for param in &tool.params {
            if param.cli_help.is_none() {
                continue;
            }
            let const_name = format!("{prefix}_{}_HELP", rust_const_name(&param.name));
            if !const_names.insert(const_name.clone()) {
                return Err(format!("duplicate CLI help constant {const_name}"));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkillConfig {
    pub workflow: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SharedContent {
    pub selector_grammar: Option<SelectorGrammar>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SelectorGrammar {
    pub forms: Vec<String>,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ToolContract {
    pub name: String,
    pub mcp_description: String,
    pub cli: CliMetadata,
    pub params: Vec<ToolParamContract>,
    pub output_schema: Option<Value>,
    pub artifacts: ArtifactRenderMetadata,
}

impl ToolContract {
    fn from_raw(name: String, raw: RawToolDef) -> Result<Self, String> {
        let mut params = raw
            .params
            .into_iter()
            .map(|param| ToolParamContract::from_raw(&name, param))
            .collect::<Result<Vec<_>, _>>()?;
        if raw.mcp_namespace_scope {
            params.extend(mcp_namespace_scope_params());
        }
        let output_schema = parse_optional_json(&name, "output_schema", raw.output_schema)?;

        Ok(Self {
            artifacts: ArtifactRenderMetadata::for_tool(&name, raw.render_cli_help),
            cli: CliMetadata {
                name: raw.cli_name,
                about: raw.cli_about,
            },
            mcp_description: raw.mcp_description,
            name,
            output_schema,
            params,
        })
    }

    pub fn tool_entry_value(&self, shared: &SharedContent) -> Value {
        let mut properties = Map::new();
        let mut required = Vec::new();
        for param in &self.params {
            properties.insert(param.name.clone(), param.schema_value(shared));
            if param.required {
                required.push(Value::String(param.name.clone()));
            }
        }

        let mut input_schema = json!({
            "additionalProperties": false,
            "type": "object",
            "properties": properties
        });
        if !required.is_empty() {
            input_schema["required"] = Value::Array(required);
        }

        let mut entry = json!({
            "name": self.name,
            "description": self.mcp_description,
            "inputSchema": input_schema
        });
        if let Some(schema) = &self.output_schema {
            entry["outputSchema"] = schema.clone();
        }
        entry
    }

    fn alias(&self, raw: RawToolAlias) -> Result<Self, String> {
        if raw.name.trim().is_empty() {
            return Err(format!("{}.mcp_aliases name must not be empty", self.name));
        }
        Ok(Self {
            artifacts: ArtifactRenderMetadata::for_alias(&raw.name),
            cli: self.cli.clone(),
            mcp_description: raw.mcp_description,
            name: raw.name,
            output_schema: self.output_schema.clone(),
            params: self.params.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct CliMetadata {
    pub name: String,
    pub about: String,
}

#[derive(Debug, Clone)]
pub struct ArtifactRenderMetadata {
    pub mcp_schema_file: String,
    pub cli_help_prefix: String,
    pub render_cli_help: bool,
}

impl ArtifactRenderMetadata {
    fn for_tool(name: &str, render_cli_help: bool) -> Self {
        Self {
            mcp_schema_file: format!("{name}.json"),
            cli_help_prefix: rust_const_name(name),
            render_cli_help,
        }
    }

    fn for_alias(name: &str) -> Self {
        Self {
            mcp_schema_file: format!("{name}.json"),
            cli_help_prefix: rust_const_name(name),
            render_cli_help: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolParamContract {
    pub name: String,
    pub required: bool,
    pub enum_values: Option<Vec<String>>,
    pub mcp_description: String,
    pub cli_help: Option<String>,
    pub cli_flag: Option<String>,
    pub selector: bool,
    shape: ParamShape,
}

impl ToolParamContract {
    fn from_raw(tool_name: &str, raw: RawParamDef) -> Result<Self, String> {
        Ok(Self {
            shape: ParamShape::from_raw(tool_name, &raw)?,
            name: raw.name,
            required: raw.required,
            enum_values: raw.enum_values,
            mcp_description: raw.mcp_description,
            cli_help: raw.cli_help,
            cli_flag: raw.cli_flag,
            selector: raw.selector,
        })
    }

    pub fn schema_value(&self, shared: &SharedContent) -> Value {
        let mut schema = self.shape.schema_object();
        let description = if self.selector {
            let selector_help = render_selector_grammar_block(shared)
                .expect("shared.selector_grammar exists for selector MCP params");
            format!("{}\n\n{selector_help}", self.mcp_description)
        } else {
            self.mcp_description.clone()
        };
        schema.insert("description".to_string(), Value::String(description));
        if let Some(values) = &self.enum_values {
            schema.insert(
                "enum".to_string(),
                Value::Array(values.iter().cloned().map(Value::String).collect()),
            );
        }
        Value::Object(schema)
    }
}

pub fn render_selector_grammar_block(shared: &SharedContent) -> Option<String> {
    let grammar = shared.selector_grammar.as_ref()?;
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
    Some(lines.join("\n"))
}

#[derive(Debug, Clone)]
enum ParamShape {
    Scalar(String),
    Array(String),
    Custom(Value),
}

impl ParamShape {
    fn from_raw(tool_name: &str, raw: &RawParamDef) -> Result<Self, String> {
        if let Some(schema) = &raw.mcp_schema {
            return parse_json(tool_name, &raw.name, schema).map(Self::Custom);
        }
        if raw.type_ == "array" {
            return Ok(Self::Array(
                raw.items_type
                    .clone()
                    .unwrap_or_else(|| "string".to_string()),
            ));
        }
        Ok(Self::Scalar(raw.type_.clone()))
    }

    fn schema_object(&self) -> Map<String, Value> {
        match self {
            Self::Scalar(kind) => {
                Map::from_iter([("type".to_string(), Value::String(kind.clone()))])
            }
            Self::Array(item_kind) => Map::from_iter([
                ("type".to_string(), Value::String("array".to_string())),
                ("items".to_string(), json!({ "type": item_kind })),
            ]),
            Self::Custom(value) => value.as_object().cloned().unwrap_or_default(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawToolsToml {
    #[serde(default)]
    shared: Option<SharedContent>,
    skill: Option<SkillConfig>,
    tools: IndexMap<String, RawToolDef>,
}

#[derive(Debug, Deserialize)]
struct RawToolDef {
    cli_name: String,
    mcp_description: String,
    cli_about: String,
    #[serde(default = "default_render_cli_help")]
    render_cli_help: bool,
    output_schema: Option<String>,
    #[serde(default)]
    mcp_namespace_scope: bool,
    #[serde(default)]
    params: Vec<RawParamDef>,
    #[serde(default)]
    mcp_aliases: Vec<RawToolAlias>,
}

#[derive(Debug, Deserialize)]
struct RawParamDef {
    name: String,
    #[serde(rename = "type")]
    type_: String,
    #[serde(default)]
    required: bool,
    items_type: Option<String>,
    enum_values: Option<Vec<String>>,
    mcp_description: String,
    cli_help: Option<String>,
    cli_flag: Option<String>,
    #[serde(default)]
    selector: bool,
    mcp_schema: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawToolAlias {
    name: String,
    mcp_description: String,
}

fn default_render_cli_help() -> bool {
    true
}

fn mcp_namespace_scope_params() -> Vec<ToolParamContract> {
    vec![
        ToolParamContract {
            name: "namespace".to_string(),
            required: false,
            enum_values: None,
            mcp_description: "Namespace slug to scope this read. Overrides the caller session namespace fallback.".to_string(),
            cli_help: None,
            cli_flag: None,
            selector: false,
            shape: ParamShape::Scalar("string".to_string()),
        },
        ToolParamContract {
            name: "all_namespaces".to_string(),
            required: false,
            enum_values: None,
            mcp_description: "Bypass namespace scoping and read across all namespaces.".to_string(),
            cli_help: None,
            cli_flag: None,
            selector: false,
            shape: ParamShape::Scalar("boolean".to_string()),
        },
    ]
}

fn parse_optional_json(
    tool_name: &str,
    field: &'static str,
    raw: Option<String>,
) -> Result<Option<Value>, String> {
    raw.map(|schema| parse_json(tool_name, field, &schema))
        .transpose()
}

fn parse_json(tool_name: &str, field: &str, raw: &str) -> Result<Value, String> {
    let value: Value = serde_json::from_str(raw)
        .map_err(|error| format!("{tool_name}.{field} is not valid JSON: {error}"))?;
    if !value.is_object() {
        return Err(format!("{tool_name}.{field} must be a JSON object"));
    }
    Ok(value)
}

pub fn rust_const_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_registry_preserves_committed_tool_order() {
        let names = contract_registry()
            .tools()
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "session_run",
                "agent_run",
                "session_list",
                "agent_list",
                "session_get",
                "agent_get",
                "namespace_list",
                "namespace_get",
                "session_capture",
                "agent_capture",
                "session_delete",
                "agent_delete",
                "session_label",
                "agent_label",
                "mail_send",
                "mail_read",
                "mail_check",
                "mail_stop_check",
                "nudge",
                "logs",
                "wait",
                "doctor",
            ]
        );
    }

    #[test]
    fn duplicate_final_mcp_tool_names_are_rejected() {
        let error = ToolContractRegistry::from_toml_str(
            r#"
[tools.first]
cli_name = "first"
mcp_description = "first tool"
cli_about = "first tool"

[[tools.first.mcp_aliases]]
name = "second"
mcp_description = "duplicate alias"

[tools.second]
cli_name = "second"
mcp_description = "second tool"
cli_about = "second tool"
"#,
        )
        .expect_err("duplicate alias should fail");

        assert!(error.contains("duplicate MCP tool name second"));
    }

    #[test]
    fn parse_error_points_to_tool_source_layout() {
        let error =
            ToolContractRegistry::from_toml_str("[tools").expect_err("invalid TOML should fail");

        assert!(error.contains("failed to parse tools/*.toml"));
    }

    #[test]
    fn duplicate_cli_help_constants_are_rejected() {
        let error = ToolContractRegistry::from_toml_str(
            r#"
[tools.foo_bar]
cli_name = "foo-bar"
mcp_description = "first tool"
cli_about = "first tool"

[tools.foo-bar]
cli_name = "foo bar"
mcp_description = "second tool"
cli_about = "second tool"
"#,
        )
        .expect_err("duplicate generated constant should fail");

        assert!(error.contains("duplicate CLI help constant FOO_BAR_ABOUT"));
    }

    #[test]
    fn render_cli_help_false_skips_cli_constant_collision() {
        let registry = ToolContractRegistry::from_toml_str(
            r#"
[tools.foo_bar]
cli_name = "foo-bar"
mcp_description = "first tool"
cli_about = "first tool"

[tools.foo-bar]
cli_name = "foo bar"
mcp_description = "second tool"
cli_about = "second tool"
render_cli_help = false
"#,
        )
        .expect("non-rendered CLI help does not collide");

        let skipped = registry
            .tools()
            .iter()
            .find(|tool| tool.name == "foo-bar")
            .expect("second tool is present");
        assert!(!skipped.artifacts.render_cli_help);
    }
}
