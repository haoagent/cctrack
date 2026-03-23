.PHONY: build test run clean release

CARGO := cargo

build:
	$(CARGO) build

test:
	$(CARGO) test

run:
	$(CARGO) run

release:
	$(CARGO) build --release
	@ls -lh target/release/cctrack
	@echo "Binary size: $$(du -h target/release/cctrack | cut -f1)"

clean:
	$(CARGO) clean

fmt:
	$(CARGO) fmt

clippy:
	$(CARGO) clippy -- -D warnings
