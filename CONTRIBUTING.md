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

```bash
rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-gnu
cargo clippy -p skarn-sandbox --target x86_64-unknown-linux-gnu --all-targets -- -D warnings
cargo clippy -p skarn-sandbox --target x86_64-pc-windows-gnu  --all-targets -- -D warnings
```

## Guidelines

- **Match the surrounding code.** Comment density, naming, and idioms should look
  like the file you're editing.
- **Tests are not optional** for behavior changes. The sandbox has runtime tests
