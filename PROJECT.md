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

Selector arguments follow one shape rule. Batch mutation commands take
positional selectors: `sm delete session <SELECTOR>` and
`sm label <SELECTOR> <MUTATION>`. Single-session commands take positional
session ids: `sm capture <SESSION_ID>`. Session list and read commands take
selectors through `--selector`: `sm get session --selector <SELECTOR>`.

Selector matching is separate from namespace scoping. `namespace:default` is a
selector that matches sessions in the default namespace. `--namespace default`
and `-A` control the namespace scope used while resolving selectors. Namespace
reads are not selector-consuming; `sm get namespace` lists namespaces and
`sm get namespace <slug>` reads one namespace by slug.

Namespace context precedence is explicit `--namespace`, `SM_NAMESPACE`, user
context from `sm config set-context`, then `default`. Deleting a namespace
cascades to its sessions and clears the user context when it points at the
deleted namespace. Namespaces cannot be renamed, and sessions cannot move between
namespaces. Stop and respawn the session in the desired namespace.

Unmanaged-session adoption is deferred to the `schedule-matters` Linear project:
https://linear.app/alphabio/project/schedule-matters. A retired adoption
command should return only with a coherent reconcile model such as import,
adopt, or scheduler-owned binding.
