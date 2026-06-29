# Compliant Confidential Proof-of-Reserves on Stellar (RISC Zero + Soroban)

Prove an issuer is **solvent** (assets >= liabilities) and verify it **on-chain on
Stellar** — *without revealing any individual balance* — with two compliance
features that make it real-world ready:

- **Selective disclosure (view key):** the public sees only **"solvent ✓"**. The
  exact collateralization ratio is **encrypted on-chain**; only an auditor holding
  the view key can decrypt it. The zero-knowledge proof guarantees the ciphertext
  encrypts the *true* computed ratio.
- **Merkle inclusion proofs:** the proof commits a Merkle root over customer
  liabilities, so **any customer can prove on-chain that their balance was counted**
  in the solvency total. Tampered proofs are rejected.

Built for **Stellar Hacks: Real-World ZK**. This is the "compliant privacy" pattern
the hackathon highlights as Stellar's sweet spot: prove the public fact, encrypt the
sensitive detail, give regulators a key, and let everyone verify.

> A note on honesty: a basic version (just a public ratio) is tagged `v1-working`.
> This `main` branch is the upgraded compliant/confidential version.

---

## Why this matters

When an exchange or stablecoin issuer holds your money, you trust they actually have
it. When that trust breaks (FTX, Celsius), people lose everything. The naive fix —
publishing everyone's balances — destroys privacy. ZK lets you prove solvency while
revealing nothing; selective disclosure then gives auditors lawful visibility without
exposing the public to private data. That mirrors how Binance's proof-of-reserves
works (ZK on liabilities, public assets) — except here **Stellar itself verifies the
proof**, which Protocol 25/26 ZK host functions newly make affordable.

## How the ZK is load-bearing

The guest ([`por/methods/guest/src/main.rs`](por/methods/guest/src/main.rs)) asserts:

```rust
assert!(total_assets >= total_liabilities, "INSOLVENT");
```

A valid proof for this program's `image_id` can therefore only exist if the issuer is
solvent. The guest then (1) builds a Merkle root over liabilities and (2) encrypts the
ratio under the view key — committing `solvent`, `enc_ratio`, `merkle_root`, etc. to
the public journal. Individual balances are private inputs that never leave the prover.

## Architecture

```
  OFF-CHAIN (prover)                         ON-CHAIN (Stellar testnet)
  private: assets[], liabilities[], view_key
        | RISC Zero zkVM
  assert solvent; build merkle_root;         ProofOfReserves (our contract)
  encrypt ratio; commit public journal        - require image_id == ours
        | Groth16 wrap                         - sha256(journal); router.verify()
  seal / image_id / journal  --------------->  - record solvent + enc_ratio + root
                                               - verify_inclusion(leaf,index,path)
                                                      | cross-contract verify()
                                               Nethermind RISC Zero verifier stack
                                               (Router -> Groth16Verifier, BN254)
  off-chain tools:
   - auditor_decrypt.py  (view key -> true ratio)
   - merkle_proof.py     (customer inclusion proof)
```

## Live on Stellar Testnet

| Component | Contract ID |
|---|---|
| **ProofOfReserves v2** | `CBVV3KMSJA6JGMWHSODTMZVWQFVCHTHNWH3MVW5ARIJWWBJP2RHWE4KE` |
| VerifierRouter | `CDZG66I3HWXFNZW7BKAZH2C4R5BZMCFVBXNVF4IDNRHDL3XYNS44W2Y5` |
| Groth16Verifier | `CC3YPLTUQYRCA3MQDXIXCAXJAPLXJIHKP5OVRRLOCOGCVUSYSJ4YWJTB` |

- Guest `image_id`: `79c3b2c1568d89550d7cf918d4764fbdaa43dc7da50b8cec67cb24ac59a949a4`
- Live `submit` tx: [`4958b7f7…`](https://stellar.expert/explorer/testnet/tx/4958b7f7d78946df75a86cdd395d8a18e14d6ea9fe2677a76e0386461a6dcdfb)
- Full address list: [DEPLOYMENTS.md](DEPLOYMENTS.md)

What's stored on-chain after a successful submit:
```json
{ "solvent": true, "enc_ratio": 2805658215177317376,
  "merkle_root": "64c92872...", "n_accounts": 3, "statement_ts": 1750000000 }
```
Note `enc_ratio` is the **ciphertext** — the public never sees 122.72%; only the
auditor's view key recovers it.

## Run the demo

```bash
bash scripts/demo_fast.sh
```
It submits the proof (verifies on-chain), shows the public view, decrypts the ratio as
an auditor, and proves a customer's inclusion on-chain. To regenerate the proof from
scratch: `cd por && cargo run -p host`.

## Repo layout

```
por/                          RISC Zero zkVM (guest = solvency + merkle + encryption)
contracts/proof-of-reserves/  Soroban contract (submit, verify_inclusion, selective disclosure)
scripts/auditor_decrypt.py    auditor tool: view key -> true ratio
scripts/merkle_proof.py       customer tool: Merkle inclusion proof
scripts/demo_fast.sh          one-command end-to-end demo
DEPLOYMENTS.md                all deployed addresses
```

## Honest notes / limitations

- **Demo data.** Issuer books in `por/host/src/main.rs` are hardcoded demo values.
- **Input-truth anchor.** ZK proves the computation, not that the inputs are the
  issuer's real books. Production needs: public on-chain asset addresses, the Merkle
  root cross-checked against real customer attestations, and signed reserves for
  off-chain assets. The Merkle inclusion here is the customer-side half of that.
- **Simplified view-key crypto.** The ratio is encrypted with a sha256-keystream XOR
  (a one-time pad keyed by `view_key || timestamp`). It demonstrates selective
  disclosure cleanly; production would use authenticated encryption / a proper KEM.
- **Snapshot, not continuous; not audited; testnet only.**