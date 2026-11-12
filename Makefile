AGENT_X86 := $(CURDIR)/target/x86_64-unknown-linux-musl/release/zerowol-agent
AGENT_ARM := $(CURDIR)/target/aarch64-unknown-linux-musl/release/zerowol-agent

.PHONY: test dev agents dist image smoke ui-regression

DEV_DATA ?= $(CURDIR)/target/dev-data
DEV_PORT ?= 8090

test:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test --workspace
	cargo clippy -p zerowol --all-targets --features dev-fakes -- -D warnings
	cargo test -p zerowol --features dev-fakes fake::

# Development server with simulated fake machines (never in release builds).
dev:
	mkdir -p $(DEV_DATA)
	ZEROWOL_DATA=$(DEV_DATA) ZEROWOL_PORT=$(DEV_PORT) ZEROWOL_DEV_FAKES=1 \
		cargo run -p zerowol --features dev-fakes

agents:
	cargo zigbuild --release -p zerowol-agent --target x86_64-unknown-linux-musl
	cargo zigbuild --release -p zerowol-agent --target aarch64-unknown-linux-musl

dist: agents
	ZEROWOL_AGENT_X86_64=$(AGENT_X86) ZEROWOL_AGENT_AARCH64=$(AGENT_ARM) \
		cargo zigbuild --release -p zerowol --target x86_64-unknown-linux-musl
	mkdir -p dist
	cp target/x86_64-unknown-linux-musl/release/zerowol dist/zerowol

image:
	docker build -t zerowol:latest .

smoke:
	scripts/e2e-smoke.sh

# Browser UI regression cases (needs Playwright, see the script header).
ui-regression:
	scripts/ui-regression.sh
