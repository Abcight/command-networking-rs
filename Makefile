setup:
	rustup target add wasm32-unknown-unknown
	cargo install basic-http-server

run:
	cargo run --release

build:
	cargo build --release

wasm:
	cargo build --release --target wasm32-unknown-unknown

serve:
	make wasm
	basic-http-server .