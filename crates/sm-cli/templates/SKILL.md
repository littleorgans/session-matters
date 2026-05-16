---
name: session-matters
description: Control local Helioy agent sessions through smd via MCP tools.
---

# session-matters

Use this skill when you need to spawn, list, inspect, or terminate local Helioy agent sessions.

## MCP Tools

| Tool | CLI | Purpose |
|------|-----|---------|
| `agent_run` | `sm run` | Start an agent runtime through the session-matters daemon. This v1 pass supports claude and codex runtimes, a role, and a workspace. The tool returns the persisted session record. |
| `agent_list` | `sm get agents` | List session records known to the session-matters daemon. Pass an id only when a narrow list response is useful; use agent_get when exactly one session is required. |
| `agent_get` | `sm get agent` | Get one session record by id. The tool returns an error envelope when the id is unknown. |
| `agent_delete` | `sm delete agent` | Terminate one daemon owned agent runtime by id and return the updated session record. Defaults to SIGTERM with a five second grace period. |

## Examples

### `agent_run`

```json
{
  "role": "engineer",
  "runtime": "claude",
  "workspace": "session-matters"
}
```

### `agent_list`

```json
{}
```

### `agent_get`

```json
{
  "id": "019e32e3-0000-7000-8000-000000000000"
}
```

### `agent_delete`

```json
{
  "grace_secs": 5,
  "id": "019e32e3-0000-7000-8000-000000000000",
  "signal": "SIGTERM"
}
```


## Session Control Workflow

Use `agent_run` to start a local agent runtime through the session-matters daemon.
Use `agent_list` to inspect live and terminated sessions.
Use `agent_get` before acting on one session id.
Use `agent_delete` to terminate daemon owned sessions.
