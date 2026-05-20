# Changelog

## [0.2.1](https://github.com/littleorgans/session-matters/compare/v0.2.0...v0.2.1) (2026-05-20)


### Features

* add --version stamp and align install/check pipelines with playbook ([#5](https://github.com/littleorgans/session-matters/issues/5)) ([f0d0587](https://github.com/littleorgans/session-matters/commit/f0d0587dcf083253352e337b947460adc581ed5d))
* ship session-matters v1 ([#1](https://github.com/littleorgans/session-matters/issues/1)) ([be655b5](https://github.com/littleorgans/session-matters/commit/be655b582a1dba815a05d79c2d86d3d1685355d0))


### Bug Fixes

* drop unsupported Windows release target ([#3](https://github.com/littleorgans/session-matters/issues/3)) ([03a2402](https://github.com/littleorgans/session-matters/commit/03a24024d00681c1e6b5880ebff45b6dd51ce0f3))

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
