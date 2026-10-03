.PHONY: setup check run

# The hooks first: .githooks/pre-push also runs without mise.
setup:
	git config core.hooksPath .githooks
	if command -v mise >/dev/null 2>&1; then mise trust; fi

check:
	.githooks/pre-push

run:
	cargo run
