# rust-esplora-client

Bitcoin Esplora API client library. Supports plaintext, TLS and Onion servers. Blocking or async.

<p>
    <a href="https://crates.io/crates/esplora-client"><img src="https://img.shields.io/crates/v/esplora-client.svg"/></a>
    <a href="https://docs.rs/esplora-client"><img src="https://img.shields.io/badge/docs.rs-esplora--client-blue"/></a>
    <a href="https://blog.rust-lang.org/2023/12/28/Rust-1.75.0.html"><img src="https://img.shields.io/badge/MSRV-1.75.0%2B-orange.svg"/></a>
    <a href="https://github.com/bitcoindevkit/rust-esplora-client/blob/master/LICENSE.md"><img src="https://img.shields.io/badge/License-MIT%2FApache--2.0-red.svg"/></a>
    <a href="https://github.com/bitcoindevkit/rust-esplora-client/actions/workflows/cont_integration.yml"><img src="https://github.com/bitcoindevkit/rust-esplora-client/actions/workflows/cont_integration.yml/badge.svg"></a>
</p>

## Minimum Supported Rust Version (MSRV)

This library should compile with any combination of features with Rust 1.75.0.

To build with the MSRV you will need to pin dependencies:

```shell
bash ci/pin-msrv.sh
```

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
