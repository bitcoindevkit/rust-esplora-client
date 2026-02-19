#!/bin/bash

set -x
set -euo pipefail

# Pin dependencies for MSRV (1.63.0)
cargo update -p reqwest --precise "0.12.1"
cargo update -p hyper --precise "1.8.0"
cargo update -p futures-channel --precise "0.3.31"
cargo update -p futures-util --precise "0.3.31"
cargo update -p futures-core --precise "0.3.31"
cargo update -p futures-task --precise "0.3.31"
cargo update -p minreq --precise "2.13.2"
cargo update -p home --precise "0.5.5"
cargo update -p url --precise "2.5.0"
cargo update -p tokio --precise "1.38.1"
cargo update -p native-tls --precise "0.2.13"
cargo update -p security-framework-sys --precise "2.11.1"
cargo update -p ring --precise "0.17.12"
cargo update -p flate2 --precise "1.0.35"
cargo update -p once_cell --precise "1.20.3"
cargo update -p tracing --precise "0.1.41"
cargo update -p tracing-core --precise "0.1.33"
cargo update -p parking_lot --precise "0.12.3"
cargo update -p parking_lot_core --precise "0.9.10"
cargo update -p lock_api --precise "0.4.12"
cargo update -p socket2@0.6.2 --precise "0.5.10"
cargo update -p webpki-roots@1.0.6 --precise "1.0.0"
cargo update -p openssl --precise "0.10.73"
cargo update -p openssl-sys --precise "0.9.109"
cargo update -p syn --precise "2.0.106"
cargo update -p quote --precise "1.0.41"
cargo update -p log --precise "0.4.28"
cargo update -p itoa --precise "1.0.15"
cargo update -p serde_json --precise "1.0.145"
cargo update -p ryu --precise "1.0.20"
cargo update -p proc-macro2 --precise "1.0.103"
cargo update -p getrandom@0.4.1 --precise "0.3.4"
cargo update -p getrandom@0.2.17 --precise "0.2.15"
cargo update -p anyhow --precise "1.0.100"
cargo update -p hyper-util --precise "0.1.19"
cargo update -p unicode-ident --precise "1.0.22"

cargo update -p bzip2-sys@0.1.13+1.0.8 --precise "0.1.12+1.0.8" # dev-dependency
