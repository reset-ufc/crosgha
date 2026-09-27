.PHONY: setup check run

setup:
	git config core.hooksPath .githooks

check:
	.githooks/pre-push

run:
	cargo run
