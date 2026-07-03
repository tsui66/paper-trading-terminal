.PHONY: test lint build ci install-local package

test:
	cargo test
	./scripts/test/test_cli.sh

lint:
	cargo fmt --check
	cargo clippy -- -D warnings

ci: lint test

build:
	cargo build --release

install-local: build
	@echo "Installing to /usr/local/bin/paper"
	sudo cp target/release/paper /usr/local/bin/paper

# Host tarball for local smoke tests (matches release artifact naming)
package: build
	@./scripts/package_release.sh