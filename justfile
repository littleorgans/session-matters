default:
    @just --list

build:
    cargo build --workspace

release-build:
    cargo build --workspace --release

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
