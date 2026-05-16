# session-matters

Control plane for Helioy agent sessions.

Pass 1 proves the tracer bullet:

```bash
sm daemon start
sm run claude --role general --workspace test --detach
sm get agents
sm daemon stop
```

The daemon uses `~/.sm/sm.pid`, `~/.sm/sock`, and `~/.sm/sm.db` by default.
Set `SM_HOME` to use an alternate runtime directory.
