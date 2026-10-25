#!/bin/bash

set -x
set -euo pipefail

# Pin dependencies for MSRV (1.80.0)
cargo update -p url --precise "2.5.0"
cargo update -p reqwest --precise "0.12.1"
cargo update -p hyper-util --precise "0.1.20"
cargo update -p native-tls --precise "0.2.13"
cargo update -p home --precise "0.5.9"
cargo update -p tokio --precise "1.53.3"
cargo update -p jobserver --precise "0.1.34"
cargo update -p getrandom@0.4 --precise "0.3.4"
cargo update -p zeroize --precise "1.8.2"
