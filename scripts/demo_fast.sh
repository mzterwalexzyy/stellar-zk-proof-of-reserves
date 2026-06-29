#!/usr/bin/env bash
# Compliant Confidential Proof-of-Reserves — full demo on Stellar testnet.
# Uses the already-generated proof (no 4-min re-proving).
set -euo pipefail
cd "$(dirname "$0")/.."

POR=CBVV3KMSJA6JGMWHSODTMZVWQFVCHTHNWH3MVW5ARIJWWBJP2RHWE4KE
SEAL=$(sed -n '1p' por/proof.txt)
IMG=$(sed -n '2p' por/proof.txt)
JOURNAL=$(cat por/journal.hex)
VIEWKEY=$(cat por/viewkey.hex)

echo "=================================================================="
echo "  COMPLIANT CONFIDENTIAL PROOF-OF-RESERVES on STELLAR"
echo "=================================================================="
echo ""
echo "  Issuer PRIVATE books (never revealed):"
echo "     assets 1,350,000  vs  liabilities 1,100,000"
echo ""
echo "  >> Submitting the zk proof to Stellar (verifies on-chain)..."
REC=$(stellar contract invoke --network testnet --source issuer --id "$POR" -- \
        submit --seal "$SEAL" --image_id "$IMG" --journal "$JOURNAL" 2>/dev/null)
echo "$REC" | python3 -m json.tool
ENC=$(echo "$REC" | python3 -c "import sys,json;print(json.load(sys.stdin)['enc_ratio'])")
TS=$(echo "$REC" | python3 -c "import sys,json;print(json.load(sys.stdin)['statement_ts'])")

echo ""
echo "  [1] PUBLIC sees only:  solvent = TRUE.  The ratio is ENCRYPTED on-chain."
echo "      enc_ratio = $ENC   (meaningless without the view key)"
echo ""
echo "  [2] AUDITOR with the view key decrypts the true ratio:"
echo -n "      -> "; python3 scripts/auditor_decrypt.py "$VIEWKEY" "$ENC" "$TS"
echo ""
echo "  [3] CUSTOMER #0 proves their balance was counted (Merkle inclusion):"
MP=$(python3 scripts/merkle_proof.py 0)
LEAF=$(echo "$MP" | awk '/^leaf/{print $3}')
PATH_JSON=$(echo "$MP" | sed -n 's/^path  = //p')
echo "      leaf = ${LEAF:0:24}...  index = 0"
echo -n "      on-chain verify_inclusion -> "
stellar contract invoke --send=no --network testnet --source issuer --id "$POR" -- \
  verify_inclusion --leaf "$LEAF" --index 0 --path "$PATH_JSON" 2>/dev/null
echo ""
echo "  Contract: https://stellar.expert/explorer/testnet/contract/$POR"
echo "=================================================================="