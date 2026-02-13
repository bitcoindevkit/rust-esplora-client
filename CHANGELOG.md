# Changelog

All notable changes to this project can be found here and in each release's git tag and can be viewed with `git tag -ln100 "v*"`.

Contributors do not need to change this file but do need to add changelog details in their PR descriptions. The person making the next release will collect changelog details from included PRs and edit this file prior to each release.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v0.12.3]

### Added

- feat(client): add `get_block_infos` #164

### Changed

- chore(client): deprecate `get_blocks` #164
- chore: use the `alloc` feature on `serde_json #168

## [v0.12.2]

### Added

- feat: add new `get_address_utxos` method #134
- feat: add new `Utxo` and `UtxoStatus` API types #134
- feat: add justfile #140
- feat(api): add `ScriptHashTxsSummary` and `ScriptHashStats` structs #143
- feat(api): add `BlockInfo` struct #143
- feat(api): add `MempoolStats` struct #143
- feat(api): add `MempoolRecentTx` struct #143
- feat(client): add `get_tx_outspends` method (`GET /tx/:txid/outspends`) #143
- feat(client): add `get_scripthash_stats` method (`GET /scripthash/:hash`) #143
- feat(client): add `get_mempool_address_txs` method (`GET /address/:address/txs/mempool`) #143
- feat(client): add `get_mempool_scripthash_txs` method (`GET /scripthash/:hash/txs/mempool`) #143
- feat(client): add `get_scripthash_utxos` method (`GET /scripthash/:hash/utxo`) #143
- feat(client): add `get_block_info` method (`GET /block/:hash`) #143
- feat(client): add `get_block_txids` method (`GET /block/:hash/txids`) #143
- feat(client): add `get_block_txs` method (`GET /block/:hash/txs[/:start_index]`) #143
- feat(client): add `get_mempool_stats` method (`GET /mempool`) #143
- feat(client): add `get_mempool_txids` method (`GET /mempool/txids`) #143
- feat(client): add `get_mempool_recent_txs` method (`GET /mempool/recent`) #143
- chore(docs): add missing documentation #147
- feat(client): add new `submit_package` API to `BlockingClient` and `AsyncClient` #114
- feat(api): add new `SubmitPackageResult`, `TxResult`, and `MempoolFeesSubmitPackage` API structures #114

### Changed

- fix(ci): pin dependencies to MSRV supported versions #138
- chore(deps): bump webpki-roots to 1.0.4, pin quote to 1.0.41 #139
- feat(ci): always run CI workflow #144
- fix(ci): bump pinned webpki-roots to 1.0.5 and pin other dependencies #153
- feat(client): update the `post_request_hex` method to `post_request_bytes`, now handling `query_params` and having `Response` as return type #114
- feat(client): update the internals of the  `broadcast` method to use new `post_request` and `post_request_bytes`, with no breaking change #114
- chore(submit_package): use `unwrap_or_default` instead of `.unwrap()` #159
