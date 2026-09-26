# One command a contributor runs before a pull request: `make gate`.
# CI's required `gate` check runs the README consistency steps; the demo
# workspace steps below need a Rust toolchain and run locally.

DEMOS := examples/demos

.PHONY: gate readme demos-fmt demos-clippy demos-test demos-verify

gate: readme demos-fmt demos-clippy demos-test demos-verify
	@echo "GATE OK"

readme:
	.github/scripts/check-readme-consistency.sh --self-test
	.github/scripts/check-readme-consistency.sh

demos-fmt:
	cd $(DEMOS) && cargo fmt --all -- --check

demos-clippy:
	cd $(DEMOS) && cargo clippy --workspace --all-targets -- -D warnings

demos-test:
	cd $(DEMOS) && cargo test --workspace

demos-verify:
	cd $(DEMOS) && cargo run -q -p xtask -- verify
