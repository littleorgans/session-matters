default:
    @just --list

SM_LOCAL_BIN := env_var_or_default("SM_LOCAL_BIN", "/Users/alphab/.cargo/bin/sm")

build:
    cargo build --workspace

release-build:
    cargo build --workspace --release

install-local: release-build
    @set -eu; \
    src="$(pwd)/target/release/sm"; \
    dest="{{SM_LOCAL_BIN}}"; \
    case "$dest" in /*) ;; *) dest="$(pwd)/$dest";; esac; \
    if [ "$src" = "$dest" ]; then \
        echo "Built $src"; \
    else \
        mkdir -p "$(dirname "$dest")"; \
        install -m 755 "$src" "$dest"; \
        echo "Installed $dest"; \
    fi

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

clippy:
    cargo clippy --workspace --fix --allow-dirty -- -D warnings

loc:
    scripts/check-loc-limit.sh

check: fmt clippy loc

sm *ARGS:
    cargo run -p sm-cli -- {{ARGS}}
