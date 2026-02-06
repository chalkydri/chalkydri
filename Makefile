.POSIX:

##
# Rust
target/release/chalkydri:
	cargo b -r -p chalkydri

##
# Cleanup
clean_rust:
	cargo clean

clean: clean_rust

rust: target/release/chalkydri

all: rust

.PHONY: clean_rust clean rust all
