# esplora-client WASM smoke test

This standalone workspace exercises `AsyncClient` on
`wasm32-unknown-unknown` with the Fetch API transport, using
`wasm-bindgen-test` under Node. It runs against deterministic local HTTP
fixtures and makes no external network requests.

It is a separate workspace so its WASM-only test code is not built by the
main crate's test suite.

Run it with:

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked wasm-pack --version 0.15.0

url_file=$(mktemp)
cargo run --locked --bin fixture -- --url-file "$url_file" &
fixture_pid=$!
trap 'kill "$fixture_pid"' EXIT
while ! test -s "$url_file"; do sleep 0.1; done
ESPLORA_WASM_TEST_URL=$(cat "$url_file") wasm-pack test --node --locked
```
