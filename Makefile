.POSIX:

##
# Rust
target/release/chalkydri:
	cargo b -r -p chalkydri

##
# UI
ui/src/lib/api: ui/openapi.json
	cd ui/ && bun install
	cd ui/ && bun run gen_api

ui/build: ui/src/lib/api
	cd ui/ && bun run build

##
# Cleanup
clean_rust:
	cargo clean

clean_ui:
	rm -r ui/src/lib/api/ ui/build/

clean: clean_rust clean_ui

ui: ui/build

rust: ui/build target/release/chalkydri

all: rust

.PHONY: clean_rust clean_ui clean ui rust all
