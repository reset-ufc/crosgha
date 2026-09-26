.PHONY: setup check run

setup:
	git config core.hooksPath .githooks

check:
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	cargo test

run:
	cargo run
