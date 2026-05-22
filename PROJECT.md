# session-matters

`session-matters` is the control plane for Helioy agent sessions.

The CLI is kubectl shaped: CRUD commands use resource nouns, and session is the
first class daemon record.

- `sm daemon start`
- `sm daemon status`
- `sm create namespace project-alpha`
- `sm config set-context project-alpha`
- `sm create session claude --role general --dir test`
- `sm run claude --role general --dir test --target headless --detach`
- `sm run codex --role reviewer --namespace project-alpha --target tmux:agents:0.1 --force --detach`
- `sm get sessions`
- `sm delete session id:<session-id>`
- `sm delete namespace project-alpha`
- `sm daemon stop`

Use `sm create session` for declarative headless session creation. Use `sm run`
for imperative create and bind target workflows. `sm run --force` only preempts
an occupied tmux pane.

Labels are metadata on sessions, not standalone resources. `sm label` is the
kubectl shaped exception verb for mutating that metadata. There is no
`sm create label`, `sm get label`, or `sm delete label` CRUD surface. Inspect
labels with `sm get session --show-labels`, or select sessions with the
`label:key=value` selector grammar.

Namespace context precedence is explicit `--namespace`, `SM_NAMESPACE`, user
context from `sm config set-context`, then `default`. Deleting a namespace
cascades to its sessions and clears the user context when it points at the
deleted namespace. Namespaces cannot be renamed, and sessions cannot move between
namespaces. Stop and respawn the session in the desired namespace.
