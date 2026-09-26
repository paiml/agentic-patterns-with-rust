# Course 5 demos — Agentic Patterns: Parallel Subagent ML with Rust

Every demo here is a test that someone can also record: a Rust program with
pinned inputs, machine-checked assertions, and a receipt.

| crate | what it is |
|---|---|
| `demo-kit` | the harness: exact pins, weights/fixture digests, preflight, a resident-server guard that kills its whole process group on drop, verdicts, receipts |
| `xtask` | `cargo run -p xtask -- verify` (pins + provable-contract lint for every demo) and `cargo run -p xtask -- card <demo>` (recording card) |

## Rules every demo follows

- `demo.toml` is the single source for the test, the receipt and the recording card.
- Pins are exact (`=0.69.3`). A floor such as `>=0.69.3` is refused.
- A demo is **Green** only when every assertion held and no preflight reason fired.
  A refused verb makes it `NotRun{Refused(<verb>)}`, never Green.
- `src/main.rs` opens with a `//!` docstring naming its `Provable contract:`,
  ends `main` with an assertion, and prints `contract: <name> OK`.
- Receipts are written to `$RFML5_RECEIPTS`, which must be outside this repository.

## Run

```bash
cargo test --workspace
cargo run -p xtask -- verify
```

## Demos

| demo | what it shows |
|---|---|
| `d01-serve-hello` | Serve hello: one resident Qwen 3.5 4B, health, one streamed completion |
| `d02-cold-vs-resident` | Cold runs, measured: five fresh processes, then one resident server |
| `d04-json-verdict` | Structured review verdicts from a local Qwen 3.5 4B (Plan B: refused at the pin) |
| `d07-quorum` | Heterogeneous quorum: agy + claude lanes review one diff; the apr lane is refused at the pin |
| `d08-planted-defect` | Planted-defect sweep: agy + claude lanes over a budget-sized corpus subset |
| `d09-parity-receipt` | A parity receipt: GPU against CPU, position by position |
| `d11-refusals` | Refusals are named, not silent: apr verbs and flags refused at the pin |
| `d13-reducer` | A deterministic reducer: fan out an eval, reduce to identical bytes |
| `d15-agy-teamwork` | agy teamwork: one minter, one reducer, workers report to an inbox |
| `d16-kaizen` | Kaizen: baseline vs current from committed receipts |

The workspace manifest is `Cargo.toml`; pinned inputs the demos digest live in `fixtures`.

---

<sub>Generated — do not edit by hand. Edit `example.toml` and re-render.</sub>
