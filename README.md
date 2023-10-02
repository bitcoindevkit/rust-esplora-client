# rust-esplora-client

Bitcoin Esplora API client library. Supports plaintext, TLS and Onion servers. Blocking or async.

<p>
    <a href="https://crates.io/crates/esplora-client"><img alt="Crate Info" src="https://img.shields.io/crates/v/esplora-client.svg"/></a>
    <a href="https://github.com/bitcoindevkit/rust-esplora-client/blob/master/LICENSE"><img alt="MIT Licensed" src="https://img.shields.io/badge/license-MIT-blue.svg"/></a>
    <a href="https://github.com/bitcoindevkit/rust-esplora-client/actions/workflows/cont_integration.yml"><img alt="CI Status" src="https://github.com/bitcoindevkit/rust-esplora-client/workflows/Rust/badge.svg"></a>
    <a href='https://coveralls.io/github/bitcoindevkit/rust-esplora-client?branch=master'><img src='https://coveralls.io/repos/github/bitcoindevkit/rust-esplora-client/badge.svg?branch=master' alt='Coverage Status' /></a>
    <a href="https://docs.rs/esplora-client"><img alt="API Docs" src="https://img.shields.io/badge/docs.rs-esplora--client-green"/></a>
    <a href="https://blog.rust-lang.org/2021/12/02/Rust-1.57.0.html"><img alt="Rustc Version 1.57.0+" src="https://img.shields.io/badge/rustc-1.57.0%2B-lightgrey.svg"/></a>
    <a href="https://discord.gg/d7NkDKm"><img alt="Chat on Discord" src="https://img.shields.io/discord/753336465005608961?logo=discord"></a>
</p>

## Minimum Supported Rust Version (MSRV)
This library should compile with any combination of features with Rust 1.57.0.

To build with the MSRV you will need to pin dependencies as follows:

```shell
cargo update -p tokio --precise 1.29.1
cargo update -p reqwest --precise 0.11.18
cargo update -p rustls:0.20.9 --precise 0.20.8
cargo update -p rustix --precise 0.38.6
cargo update -p rustls:0.21.7 --precise 0.21.1
cargo update -p hyper-rustls:0.24.1 --precise 0.24.0
cargo update -p rustls-webpki:0.100.3 --precise 0.100.1
cargo update -p rustls-webpki:0.101.6 --precise 0.101.1
cargo update -p tempfile --precise 3.6.0
cargo update -p h2 --precise 0.3.20
cargo update -p flate2:1.0.27 --precise 1.0.26
cargo update -p cc --precise 1.0.81
cargo update -p tokio-util --precise 0.7.8
cargo update -p time:0.3.15 --precise 0.3.13
```