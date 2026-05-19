set shell := ["bash", "-cu"]

# Fall back to $HOME/.cargo/bin/sm if SM_LOCAL_BIN is not set in the host environment
SM_LOCAL_BIN := env("SM_LOCAL_BIN", env("HOME") / ".cargo/bin/sm")

default:
    @just --list

install: install-release

build:
    cargo build --workspace

release-build:
    cargo build --workspace --release

build-local:
    SM_VERSION_INCLUDE_GIT_SHA=1 cargo build -p sm-cli --bin sm --profile install-local

build-install-release:
    SM_VERSION_INCLUDE_GIT_SHA=0 cargo build -p sm-cli --bin sm --release

install-local: build-local
    @just _install-bin target/install-local/sm

install-release: build-install-release
    @just _install-bin target/release/sm

_install-bin src:
    @set -eu; \
    src="$(pwd)/{{src}}"; \
    dest="{{SM_LOCAL_BIN}}"; \
    case "$dest" in /*) ;; *) dest="$(pwd)/$dest";; esac; \
    if [ "$src" = "$dest" ]; then \
        echo "Built $src"; \
    else \
        mkdir -p "$(dirname "$dest")"; \
        install -m 755 "$src" "$dest"; \
        echo "Installed $dest"; \
    fi; \
    "$dest" --version

test *ARGS:
    cargo nextest run --workspace {{ARGS}}

test-doc:
    cargo test --workspace --doc

bench:
    cargo build --release -p sm-cli
    SM_BENCH_BIN="{{ justfile_directory() }}/target/release/sm" cargo bench -p sm-cli --bench hot_path

dist *ARGS:
    cargo dist build {{ARGS}}

release *ARGS:
    npx --yes release-please release-pr --config-file release-please-config.json --manifest-file .release-please-manifest.json {{ARGS}}

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

clippy-fix:
    cargo clippy --fix --workspace --all-targets --allow-dirty --allow-staged -- -D warnings

check-loc:
    bash scripts/check-loc-limit.sh

check: fmt clippy-fix fmt-check check-loc clippy

sm *ARGS:
    cargo run -p sm-cli -- {{ARGS}}
