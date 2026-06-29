#!/usr/bin/env python3
"""Customer tool: build the liability Merkle tree (same scheme as the guest) and
emit an inclusion proof for one customer, ready for verify_inclusion().
Usage: merkle_proof.py <customer_index>  [balance1 balance2 ...]"""
import sys, hashlib, json

def leaf(i, bal):
    return hashlib.sha256(i.to_bytes(4, "little") + bal.to_bytes(8, "little")).digest()

def h(a, b):
    return hashlib.sha256(a + b).digest()

idx = int(sys.argv[1])
balances = [int(x) for x in sys.argv[2:]] or [200000, 500000, 400000]

# leaves
level = [leaf(i, b) for i, b in enumerate(balances)]
target = idx
path = []
while len(level) > 1:
    nxt = []
    i = 0
    while i < len(level):
        left = level[i]
        right = level[i + 1] if i + 1 < len(level) else level[i]  # duplicate last
        if i == (target - target % 2):            # our pair
            sib = right if target % 2 == 0 else left
            path.append(sib.hex())
        nxt.append(h(left, right))
        i += 2
    target //= 2
    level = nxt

print("leaf  =", leaf(idx, balances[idx]).hex())
print("index =", idx)
print("path  =", json.dumps(path))
print("root  =", level[0].hex())