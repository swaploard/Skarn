# Contributing to Skarn

Thanks for your interest! Skarn operates at the OS-kernel boundary and runs
untrusted code, so we hold a high bar for correctness and clarity.

## Getting started

```bash
git clone https://github.com/Rani367/Skarn
cd Skarn
cargo build --workspace
cargo test --workspace
```

Rust **1.95+** is required (pinned in `rust-toolchain.toml`).

## Before you open a PR

Run the same checks CI runs:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check          # licenses & advisories (install: cargo install cargo-deny)
```

If you touched the Linux or Windows sandbox backends, also type-check them from
any host:

