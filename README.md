# Agentic Patterns: Parallel Subagent ML with Rust

Code from the course on parallel subagent patterns for ML in Rust

[![gate](https://github.com/paiml/agentic-patterns-with-rust/actions/workflows/gate.yml/badge.svg)](https://github.com/paiml/agentic-patterns-with-rust/actions/workflows/gate.yml)

Course 5 of the Machine Learning in Rust specialization. This repository holds the course outline, in OUTLINE.md; the recordable demos live in examples/demos.

## Examples

| example | what it is |
|---|---|
| [`examples/demos`](examples/demos/) | Recordable demos: Rust programs with pinned inputs, machine-checked assertions and a receipt |

## Setup

The demos need only a Rust toolchain. CI builds with Rust 1.99.1 on Ubuntu 24.04, and a weekly run covers the latest stable Rust on Ubuntu and macOS.

### Linux / Ubuntu

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.99.1 --component rustfmt --component clippy
. "$HOME/.cargo/env"
cd examples/demos
cargo test --workspace --locked
cargo run -q --locked -p xtask -- verify
```

### macOS

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
. "$HOME/.cargo/env"
cd examples/demos
cargo test --workspace --locked
cargo run -q --locked -p xtask -- verify
```

---

<sub>Generated — do not edit by hand. Edit `course.toml` and re-render.</sub>
