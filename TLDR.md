# TLDR

`session-matters` is the control plane for Helioy agent sessions. One daemon
(`smd`) owns the durable session record, the selector grammar, namespaces,
durable mail, and the MCP surface. One CLI binary (`sm`) is the local control
surface. Callers ask for a session by intent; `smd` authorizes through
identity-matters and delegates the actual process to runtime-matters over
`~/.rtm/sock`.

The shape is K8s shaped on purpose. `smd` is the API server plus etcd. `sm`
is `kubectl`. Spawn requests cross the boundary into runtime-matters, which
plays kubelet and shim. session-matters never touches a process directly.

## Mental Model

A session is the unit of work. The id is a UUIDv7 minted by `smd` before any
runtime process exists, and it is the join key across identity-matters,
runtime-matters, and transport-matters.

A namespace is an operator created slug that groups sessions. `default`
always exists. A `.sm/namespace` marker scopes CLI and MCP reads by
directory walk; `--namespace` and `-A` override marker discovery.

A selector is how callers point at sessions: `all`, `id:<uuid>`,
`role:<name>`, `namespace:<slug>`, `dir:<path>`, `label:<key>=<value>`, and
`label:<key> in (a,b)`. Every multi-target verb consumes a selector.

Mail is durable; nudge is ephemeral. Mail survives daemon restarts and is
the agent-to-agent channel. Nudge is best-effort tmux delivery for live
attention.

`sm doctor` is the first command when something feels wrong. It reports
daemon health, LOST sessions, and runtime-matters reachability.
