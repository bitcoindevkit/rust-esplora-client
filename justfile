alias a := audit
alias b := build
alias c := check
alias cs := check-sigs
alias d := doc
alias do := doc-open
alias f := fmt
alias l := lock
alias t := test
alias z := zizmor
alias p := pre-push

_default:
    @echo "> rust-esplora-client"
    @echo "> Bitcoin Esplora API client library\n"
    @just --list

[doc: "Run `cargo audit`"]
audit:
    cargo audit

[doc: "Build `rust-esplora-client`"]
build:
    RBMT_LOG_LEVEL=progress cargo rbmt run build

[doc: "Check code formatting, compilation, and linting"]
check:
    RBMT_LOG_LEVEL=progress cargo rbmt fmt --check
    RBMT_LOG_LEVEL=progress cargo rbmt lint
    RBMT_LOG_LEVEL=progress cargo rbmt docs

[doc: "Checks whether all commits in this branch are signed"]
check-sigs:
    bash contrib/check-signatures.sh

[doc: "Generate documentation"]
doc:
    RBMT_LOG_LEVEL=progress cargo rbmt docs
    RBMT_LOG_LEVEL=progress cargo rbmt run doc --no-deps

[doc: "Generate and open documentation"]
doc-open:
    RBMT_LOG_LEVEL=progress cargo rbmt docs
    RBMT_LOG_LEVEL=progress cargo rbmt run doc --no-deps --open

[doc: "Format code"]
fmt:
    RBMT_LOG_LEVEL=progress cargo rbmt fmt

[doc: "Regenerate Cargo-recent.lock and Cargo-minimal.lock"]
lock:
    RBMT_LOG_LEVEL=verbose cargo rbmt lock

[doc: "Run tests"]
test:
    RBMT_LOG_LEVEL=verbose cargo rbmt test

[doc: "Run tests with the toolchain + lockfile matrix"]
test-matrix:
    RBMT_LOG_LEVEL=verbose cargo rbmt test --toolchain stable --lock-file recent
    RBMT_LOG_LEVEL=verbose cargo rbmt test --toolchain msrv --lock-file minimal

[doc: "Run Zizmor Static Analysis"]
zizmor:
    uvx zizmor .

[doc: "Run pre-push checks"]
pre-push:
    @just lock
    @just check
    @just doc
    @just test
    @just audit
    @just zizmor
    @just check-sigs
