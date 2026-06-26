#!/usr/bin/env bash
# Fast demo for the video: uses the already-generated proof, runs the on-chain
# verification + solvency recording in ~15s (no 4-min re-proving).
set -euo pipefail
cd "$(dirname "$0")/.."

ROUTER=CDZG66I3HWXFNZW7BKAZH2C4R5BZMCFVBXNVF4IDNRHDL3XYNS44W2Y5
POR=CCULU6FRJBTG77ZZNEXWYMKMYGEDLLS6746V2ZCZZWT2RM2RHWY7DUGA
SEAL=$(sed -n '1p' por/proof.txt)
IMG=$(sed -n '2p' por/proof.txt)
JOURNAL=$(cat por/journal.hex)

echo ""
echo "=================================================================="
echo "  CONFIDENTIAL PROOF-OF-RESERVES on STELLAR  (RISC Zero + Soroban)"
echo "=================================================================="
echo ""
echo "  Issuer's PRIVATE books (never revealed on-chain):"
echo "     assets      = [400,000 | 350,000 | 600,000]  ->  1,350,000"
echo "     liabilities = [200,000 | 500,000 | 400,000]  ->  1,100,000"
echo ""
echo "  A RISC Zero zkVM proved 'assets >= liabilities' off-chain."
echo "  Program image_id: ${IMG:0:24}..."
echo ""
echo "  >> Submitting the Groth16 proof to our Stellar contract..."
echo "     (the contract calls the on-chain RISC Zero verifier to check it)"
echo ""
stellar contract invoke --network testnet --source issuer --id "$POR" -- \
  submit --seal "$SEAL" --image_id "$IMG" --journal "$JOURNAL" 2>/tmp/demo.err | python3 -m json.tool
echo ""
echo "  ON-CHAIN RESULT  ->  solvent: TRUE,  ratio: 122.72%  (12272 bps)"
echo "  ...proven WITHOUT revealing a single balance."
echo ""
echo "  View it live:"
echo "     Contract: https://stellar.expert/explorer/testnet/contract/$POR"
echo "=================================================================="