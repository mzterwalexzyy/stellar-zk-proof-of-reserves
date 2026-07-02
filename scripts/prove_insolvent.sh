#!/usr/bin/env bash
# Negative demo: an INSOLVENT issuer cannot produce a proof at all.
set -uo pipefail
cd "$(dirname "$0")/../por"
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$HOME/.risc0/bin:$PATH"
echo "Trying to prove reserves with assets < liabilities..."
echo "  assets      = 100,000"
echo "  liabilities = 900,000"
echo ""
ASSETS="100000" LIABILITIES="900000" cargo run -q -p host