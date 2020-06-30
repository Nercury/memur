export RUSTFLAGS=-Zsanitizer=leak RUSTDOCFLAGS=-Zsanitizer=leak
cargo test "$@"
