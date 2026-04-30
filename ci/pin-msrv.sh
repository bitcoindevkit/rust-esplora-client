#!/bin/bash

set -x
set -euo pipefail

# Pin dependencies for MSRV (1.75.0)
cargo update -p native-tls --precise "0.2.13"
cargo update -p "getrandom@0.4.2" --precise "0.3.4"
