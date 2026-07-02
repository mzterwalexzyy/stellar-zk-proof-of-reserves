#!/usr/bin/env bash
# End-to-end Proof-of-Reserves demo on Stellar testnet.
# Prereqs: Rust, rzup (RISC Zero), Docker, Stellar CLI, a funded `issuer` identity,
# and the Nethermind verifier cloned at ./verifier.
set -euo pipefail

ROUTER=CDZG66I3HWXFNZW7BKAZH2C4R5BZMCFVBXNVF4IDNRHDL3XYNS44W2Y5
POR=CBVV3KMSJA6JGMWHSODTMZVWQFVCHTHNWH3MVW5ARIJWWBJP2RHWE4KE

echo "==> 1. Generating Groth16 proof of reserves (private books stay private)"
( cd por && cargo run -p host )

SEAL=$(sed -n '1p' por/proof.txt)
IMG=$(sed -n '2p' por/proof.txt)
DIGEST=$(sed -n '3p' por/proof.txt)
JOURNAL=$(cat por/journal.hex)

echo "==> 2. Verifying proof directly against the router (simulation)"
stellar contract invoke --send=no --network testnet --source issuer --id "$ROUTER" -- \
  verify --seal "$SEAL" --image_id "$IMG" --journal "$DIGEST"

echo "==> 3. Submitting to ProofOfReserves (verifies on-chain + records solvency)"
stellar contract invoke --network testnet --source issuer --id "$POR" -- \
  submit --seal "$SEAL" --image_id "$IMG" --journal "$JOURNAL"

echo "==> 4. Reading the recorded attestation"
stellar contract invoke --send=no --network testnet --source issuer --id "$POR" -- latest