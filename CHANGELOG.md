# Changelog

## Unreleased

### Features

- Add namespaces as the session grouping primitive. The `default` namespace is created automatically, existing rows are backfilled into `default`, and new sessions can target a namespace only after the namespace record exists.
- Add `.sm/namespace` marker discovery with `$HOME` bounded walk up, and `sm create namespace` for operator setup.
- Add `sm run --dir` and `sm run --namespace` for spawn directory and namespace selection.
- Add `namespace:<slug>` and `dir:<path>` selectors. `workspace:<path>` selectors are removed in this migration.
- Add `session_*` MCP tools alongside deprecated `agent_*` aliases. MCP read tools accept `namespace` and `all_namespaces`; when neither is supplied, they fall back to the caller session namespace.

### Migration Notes

- CLI selector reads default to the resolved namespace from `--namespace`, `.sm/namespace`, or `default`. Use `-A` or `--all-namespaces` for cross namespace reads.
- The future hard cut master will remove compatibility surfaces such as `SpawnRequest.workspace`, `sessions.workspace`, and `agent_*` MCP aliases after the migration window.

## [0.2.3](https://github.com/littleorgans/session-matters/compare/v0.2.2...v0.2.3) (2026-05-21)


### Features

* namespace primitive + workspace→namespace dual-publish migration ([#11](https://github.com/littleorgans/session-matters/issues/11)) ([96cc491](https://github.com/littleorgans/session-matters/commit/96cc491244fae037efec397e25ac01a1e1206c60))

## [0.2.2](https://github.com/littleorgans/session-matters/compare/v0.2.1...v0.2.2) (2026-05-20)


### Bug Fixes

* 0.2.1 road-test feedback bundle ([#9](https://github.com/littleorgans/session-matters/issues/9)) ([8d54e3d](https://github.com/littleorgans/session-matters/commit/8d54e3dfeb9b1bf7d5b7414687203de4bea8ccac))

## [0.2.1](https://github.com/littleorgans/session-matters/compare/v0.2.0...v0.2.1) (2026-05-20)

Release bookkeeping only. No functional change from 0.2.0.

The auto-generated entries that originally appeared here were re-attributions of commits already shipped in 0.1.1, 0.1.2, and 0.1.3. They surfaced because the v0.2.0 release was cut manually and bypassed release-please, so the bot walked conventional commits back to its last self-managed tag (v0.1.3) when composing this changelog block.

## 0.2.0 (2026-05-20)

### Features

- Replaced the local runtime stand-in with `rtmd` as the production runtime substrate.
- Added the `sm run --target` flag for explicit headless and tmux targets.
- Added `sm capture` for tmux pane scrollback through runtime-matters.
- Added runtime-matters backed lifecycle, kill, nudge, event cursor, and doctor handling.
- Auto-link daemon-spawned headless stdout logs while keeping `sm link` for unmanaged sessions.
- Require `rtmd` with `lilo-rm` protocol 0.6 or newer at daemon startup.

### Release

- `release-please` is prepared for v0.2.0.
- `cargo dist` is configured for macOS x86 and arm, Linux GNU x86 and arm, and Linux musl x86 and arm tarballs.

## [0.1.3](https://github.com/littleorgans/session-matters/compare/v0.1.2...v0.1.3) (2026-05-19)


### Features

* add --version stamp and align install/check pipelines with playbook ([#5](https://github.com/littleorgans/session-matters/issues/5)) ([f0d0587](https://github.com/littleorgans/session-matters/commit/f0d0587dcf083253352e337b947460adc581ed5d))

## [0.1.2](https://github.com/littleorgans/session-matters/compare/v0.1.1...v0.1.2) (2026-05-17)


### Bug Fixes

* drop unsupported Windows release target ([#3](https://github.com/littleorgans/session-matters/issues/3)) ([03a2402](https://github.com/littleorgans/session-matters/commit/03a24024d00681c1e6b5880ebff45b6dd51ce0f3))

## [0.1.1](https://github.com/littleorgans/session-matters/compare/v0.1.0...v0.1.1) (2026-05-17)


### Features

* ship session-matters v1 ([#1](https://github.com/littleorgans/session-matters/issues/1)) ([be655b5](https://github.com/littleorgans/session-matters/commit/be655b582a1dba815a05d79c2d86d3d1685355d0))

## 0.1.0

- Initial v1 release line for session-matters.
