.PHONY: test lint build ci install-local package

test:
	cargo test
	./scripts/test/test_cli.sh

lint:
	cargo fmt --check
	cargo clippy -- -D warnings

ci:
	./scripts/ci.sh

build:
	cargo build --release

install-local: build
	@mkdir -p "$(HOME)/.local/bin"
	@cp target/release/paper "$(HOME)/.local/bin/paper"
	@echo "Installed to $(HOME)/.local/bin/paper (no sudo)"

# Host tarball for local smoke tests (matches release artifact naming)
package: build
	@./scripts/package_release.sh