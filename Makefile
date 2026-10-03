.PHONY: setup check run

setup:
	mise trust
	git config core.hooksPath .githooks

check:
	.githooks/pre-push

run:
	cargo run
