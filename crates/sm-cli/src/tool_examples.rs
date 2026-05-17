use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub struct ToolExample {
    pub tool: &'static str,
    pub arguments: Value,
}

pub fn examples() -> Vec<ToolExample> {
    vec![
        ToolExample {
            tool: "agent_run",
            arguments: json!({
                "runtime": "claude",
                "role": "engineer",
                "workspace": "session-matters",
                "labels": ["area=auth", "pri=high"]
            }),
        },
        ToolExample {
            tool: "agent_list",
            arguments: json!({
                "selector": "role:engineer"
            }),
        },
        ToolExample {
            tool: "agent_get",
            arguments: json!({
                "id": "019e32e3-0000-7000-8000-000000000000"
            }),
        },
        ToolExample {
            tool: "agent_delete",
            arguments: json!({
                "selector": "id:019e32e3-0000-7000-8000-000000000000",
                "signal": "SIGTERM",
                "grace_secs": 5
            }),
        },
    ]
}
