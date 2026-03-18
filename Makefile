.PHONY: build test run check watch release

build:
	cargo build

test:
	cargo test

run:
	cargo run -- check

check:
	cargo run -- check

watch:
	cargo run -- watch

release:
	cargo build --release
