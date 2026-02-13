#!/bin/bash

set -x
set -euo pipefail

# Pin dependencies for MSRV (1.75.0)
cargo update -p minreq --precise "2.13.2"
cargo update -p idna_adapter --precise "1.2.0"
cargo update -p native-tls --precise "0.2.13"
cargo update -p zerofrom --precise "0.1.5"
cargo update -p litemap --precise "0.7.4"
