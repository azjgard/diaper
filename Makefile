.PHONY: build test run check watch release release-linux e2e

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

release-linux:
	docker build -f linux-builder.Dockerfile -o target/linux-release .

e2e:
	docker build -f e2e/Dockerfile -t diaper-e2e .
	docker run --rm diaper-e2e
