#![allow(dead_code)]

use std::sync::OnceLock;

use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::{Map, Value, json};

const TOOLS_TOML: &str = include_str!("../../../tools.toml");

static REGISTRY: OnceLock<ToolContractRegistry> = OnceLock::new();

pub fn contract_registry() -> &'static ToolContractRegistry {
    REGISTRY.get_or_init(|| {
        ToolContractRegistry::from_toml_str(TOOLS_TOML)
            .unwrap_or_else(|error| panic!("tools.toml is valid: {error}"))
    })
}

#[derive(Debug, Clone)]
pub struct ToolContractRegistry {
    skill: Option<SkillConfig>,
    tools: Vec<ToolContract>,
}

impl ToolContractRegistry {
    pub fn from_toml_str(content: &str) -> Result<Self, String> {
        let parsed: RawToolsToml = toml::from_str(content)
            .map_err(|error| format!("failed to parse tools.toml: {error}"))?;
        let tools = parsed
            .tools
            .into_iter()
            .map(|(name, raw)| ToolContract::from_raw(name, raw))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            skill: parsed.skill,
            tools,
        })
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
            .map(ToolContract::tool_entry_value)
            .collect::<Vec<_>>();
        json!({ "tools": tools })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkillConfig {
    pub workflow: String,
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
        let params = raw
            .params
            .into_iter()
            .map(|param| ToolParamContract::from_raw(&name, param))
            .collect::<Result<Vec<_>, _>>()?;
        let output_schema = parse_optional_json(&name, "output_schema", raw.output_schema)?;

        Ok(Self {
            artifacts: ArtifactRenderMetadata::for_tool(&name),
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

    pub fn tool_entry_value(&self) -> Value {
        let mut properties = Map::new();
        let mut required = Vec::new();
        for param in &self.params {
            properties.insert(param.name.clone(), param.schema_value());
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
}

impl ArtifactRenderMetadata {
    fn for_tool(name: &str) -> Self {
        Self {
            mcp_schema_file: format!("{name}.json"),
            cli_help_prefix: rust_const_name(name),
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
        })
    }

    pub fn schema_value(&self) -> Value {
        let mut schema = self.shape.schema_object();
        schema.insert(
            "description".to_string(),
            Value::String(self.mcp_description.clone()),
        );
        if let Some(values) = &self.enum_values {
            schema.insert(
                "enum".to_string(),
                Value::Array(values.iter().cloned().map(Value::String).collect()),
            );
        }
        Value::Object(schema)
    }
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
    skill: Option<SkillConfig>,
    tools: IndexMap<String, RawToolDef>,
}

#[derive(Debug, Deserialize)]
struct RawToolDef {
    cli_name: String,
    mcp_description: String,
    cli_about: String,
    output_schema: Option<String>,
    #[serde(default)]
    params: Vec<RawParamDef>,
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
    mcp_schema: Option<String>,
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
