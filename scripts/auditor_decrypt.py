#!/usr/bin/env python3
"""Auditor tool: decrypt the confidential ratio using the view key.
Usage: auditor_decrypt.py <viewkey_hex> <enc_ratio_u64> <statement_ts>
The public can read enc_ratio + statement_ts on-chain, but only the view-key
holder can recover the true ratio. The ZK proof guarantees enc_ratio encrypts
the TRUE computed ratio."""
import sys, hashlib

view_key = bytes.fromhex(sys.argv[1])
enc_ratio = int(sys.argv[2])
ts = int(sys.argv[3])

ks = hashlib.sha256(view_key + ts.to_bytes(8, "little")).digest()[:8]
keystream = int.from_bytes(ks, "little")
ratio = enc_ratio ^ keystream
print(f"decrypted ratio_bps: {ratio}  ({ratio/100:.2f}% collateralized)")