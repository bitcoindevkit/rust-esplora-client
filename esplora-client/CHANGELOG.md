# Changelog

## [Unreleased]

### Added

### Changed

* feat: split types into a new `esplora-types` crate, re-exported as `esplora_client::api` [#XXX]
* chore!: remove deprecated `BlockSummary` and `get_block` [#225]

### Fixed

## [0.13.0] – 2026-06-28

### Added

* feat(client): add missing `get_block_infos` method to `BlockingClient` and `AsyncClient` [#216]
* feat(error): implement `Display` and `std::error::Error` for `Error` [#209]
* feat(lib)!: add `Error::SerdeJson` variant [#171]
* feat(test): roll a custom `TestEnv` and drop `lazy_static` [#176]
* feat(ci): add `cargo-rbmt`-based Rust CI, signature checks, Zizmor, Dependabot, and vendored `setup-rbmt` action [#203]
* ci(audit): add new `cargo audit` job [#177]
* feat(doc): add SPDX headers and dual-license documentation [#188]

### Changed

* ci(msrv)!: bump Rust MSRV to 1.75.0 [#154]
* feat(client)!: update `broadcast` API to return the broadcast `Txid` [#151]
* refactor(client)!: use `bitreq` for both `BlockingClient` and `AsyncClient` [#137]
* refactor(client)!: rename `scripthash_txs` to `get_scripthash_txs` [#189]
* refactor(client)!: use `bitcoin::FeeRate` for `submit_package` and `get_fee_estimates` [#198]
* refactor(client)!: use `Duration` for timeout [#220]
* refactor(api)!: remove `PrevOut` in favor of `Vout` [#190]
* refactor(api)!: rename `Tx` to `EsploraTx` and add conversions [#196]
* refactor(api)!: use `bitcoin::Amount` for bitcoin amount fields [#195]
* refactor(api)!: use `bitcoin::Weight` for weight fields [#194]
* refactor(api)!: use `bitcoin::{Amount, FeeRate}` for `MempoolStats` [#198]
* refactor(api)!: use `bitcoin::Witness` for `Vin::witness` [#220]
* refactor(api)!: use `bitcoin::Sequence` for `Vin::sequence` [#220]
* refactor(api)!: use `bitcoin::transaction::Version` for `EsploraTx::version` [#220]
* refactor(api)!: use `bitcoin::absolute::LockTime` for `EsploraTx::locktime` [#220]
* refactor(api)!: use `bitcoin::Address` for `AddressStats::address` [#220]
* chore: deprecate `BlockSummary` and `get_blocks` in favor of `BlockInfo` and `get_block_infos` [#197]
* chore(deps): bump `electrsd` to v0.38.0 [#193]
* chore(deps): bump `electrsd` to v0.40.0 [#215]
* chore(deps): bump `bitreq` to v0.3.7 [#215]
* chore(doc): update license metadata, badges, and license documentation [#188]
* chore(doc): update `README.md` development instructions [#203]
* doc: redocument API types, clients, and crate documentation [#219]
* chore(ci): add `cargo-rbmt` recipes, metadata, and workflow setup [#203]
* chore(ci): update `justfile`, toolchains, workflow formatting, and `cargo-rbmt` setup [#213]
* chore(test): move `TestEnv` and tests to `tests/` [#207]

### Fixed

* fix(ci): pin `hyper-rustls` to v0.27.7 for MSRV [#187]
* fix(ci): pin transitive dependencies for MSRV and add minimal/recent lockfiles [#203]
* fix(ci): resolve Zizmor warnings [#203]
* chore(client): remove `serde_json` unwraps in favor of `Error::SerdeJson` [#171]

## [0.12.2] – 2026-01-20

### Added

* feat: add new `get_address_utxos` method [#134]
* feat: add new `Utxo` and `UtxoStatus` API types [#134]
* feat: add justfile [#140]
* feat(api): add `ScriptHashTxsSummary` and `ScriptHashStats` structs [#143]
* feat(api): add `BlockInfo` struct [#143]
* feat(api): add `MempoolStats` struct [#143]
* feat(api): add `MempoolRecentTx` struct [#143]
* feat(client): add `get_tx_outspends` method (`GET /tx/:txid/outspends`) [#143]
* feat(client): add `get_scripthash_stats` method (`GET /scripthash/:hash`) [#143]
* feat(client): add `get_mempool_address_txs` method (`GET /address/:address/txs/mempool`) [#143]
* feat(client): add `get_mempool_scripthash_txs` method (`GET /scripthash/:hash/txs/mempool`) [#143]
* feat(client): add `get_scripthash_utxos` method (`GET /scripthash/:hash/utxo`) [#143]
* feat(client): add `get_block_info` method (`GET /block/:hash`) [#143]
* feat(client): add `get_block_txids` method (`GET /block/:hash/txids`) [#143]
* feat(client): add `get_block_txs` method (`GET /block/:hash/txs[/:start_index]`) [#143]
* feat(client): add `get_mempool_stats` method (`GET /mempool`) [#143]
* feat(client): add `get_mempool_txids` method (`GET /mempool/txids`) [#143]
* feat(client): add `get_mempool_recent_txs` method (`GET /mempool/recent`) [#143]
* chore(docs): add missing documentation [#147]
* feat(client): add new `submit_package` API to `BlockingClient` and `AsyncClient` [#114]
* feat(api): add new `SubmitPackageResult`, `TxResult`, and `MempoolFeesSubmitPackage` API structures [#114]

### Changed

* fix(ci): pin dependencies to MSRV supported versions [#138]
* chore(deps): bump webpki-roots to 1.0.4, pin quote to 1.0.41 [#139]
* feat(ci): always run CI workflow [#144]
* fix(ci): bump pinned webpki-roots to 1.0.5 and pin other dependencies [#153]
* feat(client): update the `post_request_hex` method to `post_request_bytes`, now handling `query_params` and having `Response` as return type [#114]
* feat(client): update the internals of the  `broadcast` method to use new `post_request` and `post_request_bytes`, with no breaking change [#114]
* chore(submit_package): use `unwrap_or_default` instead of `.unwrap()` [#159]


[Unreleased]: https://github.com/bitcoindevkit/rust-esplora-client/compare/v0.13.0...HEAD
[0.13.0]: https://github.com/bitcoindevkit/rust-esplora-client/compare/v0.12.2...v0.13.0
[0.12.2]: https://github.com/bitcoindevkit/rust-esplora-client/compare/v0.12.1...v0.12.2
