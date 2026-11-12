AGENT_X86 := $(CURDIR)/target/x86_64-unknown-linux-musl/release/zerowol-agent
AGENT_ARM := $(CURDIR)/target/aarch64-unknown-linux-musl/release/zerowol-agent

.PHONY: test agents dist image smoke

test:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test --workspace

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
