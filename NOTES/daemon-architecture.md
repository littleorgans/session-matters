# Daemon Architecture

`smd` is the session control plane. It owns session records, labels, mail, selectors, waits, and MCP bridging. It does not fork agent runtimes directly in production.

Runtime execution belongs to runtime-matters. At startup, `smd` resolves the rtmd socket from `RTM_SOCKET_PATH`, then the runtime directory, then `~/.rtm/sock`. The daemon probes `rtmd` and requires runtime protocol 0.6 or newer before accepting work.

`sm run` persists a session record and delegates process launch to `rtmd` through `RtmdDriver`. The driver passes explicit launch context, including runtime, workspace, environment, shell resume data, and the requested target. `headless` is the default target. Tmux sessions use `tmux:SESSION:WINDOW.PANE` and are validated by runtime-matters before spawn.

`rtmd` remains authoritative for process lifecycle. `smd` reconciles status during lifecycle operations and consumes runtime event batches through the durable cursor API. Running, terminated, and lost observations update the session store in a single transaction with the runtime cursor.

Headless sessions spawned by the daemon receive stdout and stderr paths from `rtmd`; `smd` persists stdout as `Session.transcript_path`, so `sm logs` works without a manual link step. `sm link` is still available for unmanaged sessions that were not spawned by `smd`.

Operational commands delegate to the same runtime boundary:

- `sm delete agent` calls rtmd kill and treats `AlreadyExited` as a successful close.
- `sm capture` calls rtmd capture for tmux backed sessions.
- `sm nudge` calls rtmd nudge and reports typed delivered, unsupported, and failed outcomes.
- `sm doctor` includes direct runtime-matters doctor output beside session-matters health.

Previous local runtime stand-ins have been removed. Test doubles remain only as unit test fixtures around the `SpawnDriver` trait.
