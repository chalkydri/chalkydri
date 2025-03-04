.POSIX:

##
# Rust
target/release/chalkydri:
	cargo b -r -p chalkydri

##
# UI
ui/src/lib/api: ui/openapi.json
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

ui: clean_ui ui/build

rust: clean_rust ui target/release/chalkydri

all: rust

.PHONY: clean_rust clean_ui clean ui rust all
