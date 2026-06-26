# Confidential Proof-of-Reserves on Stellar (RISC Zero + Soroban)

Prove an issuer is **solvent** — that total assets ≥ total liabilities — and verify
that proof **on-chain in a Stellar smart contract**, *without revealing any individual
balance*. Built for the **Stellar Hacks: Real-World ZK** hackathon.

> TL;DR: a RISC Zero zkVM program crunches an issuer's private books off-chain and
> emits a Groth16 proof. A Soroban contract verifies that proof on Stellar and records
> "solvent at ratio X%, as of time T" — the numbers stay secret, the solvency is public
> and cryptographically checked.

This is exactly the pattern major exchanges (e.g. Binance) use for proof-of-reserves —
ZK on the private liability side, public on-chain assets — except here the proof is
**verified by the chain itself**, which is what Stellar's Protocol 25/26 ZK host
functions newly make affordable.

---

## Why this matters

When you hold money with a stablecoin issuer, exchange, or custodian, you trust they
actually hold the assets to cover what they owe. When they don't (FTX, Celsius, Terra),
people lose everything. "Proof of reserves" fixes this — but the naive version (publish
every account) is a privacy disaster. ZK collapses the dilemma: **prove `assets ≥
liabilities` while revealing only the boolean and a collateralization ratio.**

## How the ZK is load-bearing

The guest program ([`por/methods/guest/src/main.rs`](por/methods/guest/src/main.rs))
does the one thing that makes this real:

```rust
assert!(total_assets >= total_liabilities, "INSOLVENT");
```

Because of that assertion, **a valid proof for this program can only exist if the issuer
is solvent.** So the mere existence of a verifying proof, for the known program
`image_id`, *is* the proof of solvency. The individual balances are private inputs that
never leave the prover; only a ratio, a timestamp, an account count, and a liability
commitment are committed to the public journal.

## Architecture

```
  OFF-CHAIN (prover)                          ON-CHAIN (Stellar testnet)
  ──────────────────                          ──────────────────────────
  private: asset[], liability[]
        │
        ▼  RISC Zero zkVM
  assert assets >= liabilities                ┌─ ProofOfReserves (our contract) ─┐
  commit(ratio_bps, ts, n, commitment)        │  submit(seal, image_id, journal) │
        │                                     │   • require image_id == ours     │
        ▼  Groth16 wrap (STARK→SNARK)         │   • journal_digest = sha256(j)   │
  seal / image_id / journal_digest  ───────►  │   • router.verify(...)  ◄────────┼─┐
                                              │   • record solvent + ratio + emit│ │
                                              └──────────────────────────────────┘ │
                                                         │ cross-contract call      │
                                              ┌──────────▼───────── Nethermind ─────▼─┐
                                              │ VerifierRouter → EmergencyStop →       │
                                              │ Groth16Verifier (BN254 pairing check)  │
                                              └────────────────────────────────────────┘
```

We reuse Nethermind's audited-pattern [stellar-risc0-verifier](https://github.com/NethermindEth/stellar-risc0-verifier)
as the on-chain verifier stack, and add our own `ProofOfReserves` application contract
on top.

## Live on Stellar Testnet

| Component | Contract ID |
|---|---|
| **ProofOfReserves (this project)** | `CCULU6FRJBTG77ZZNEXWYMKMYGEDLLS6746V2ZCZZWT2RM2RHWY7DUGA` |
| VerifierRouter | `CDZG66I3HWXFNZW7BKAZH2C4R5BZMCFVBXNVF4IDNRHDL3XYNS44W2Y5` |
| Groth16Verifier | `CC3YPLTUQYRCA3MQDXIXCAXJAPLXJIHKP5OVRRLOCOGCVUSYSJ4YWJTB` |
| EmergencyStop | `CBN3HEDZLSS2EZQGCGPHHFBHCGUPAJJZBCPECXMWV2JRG2RKF4NAFM4V` |
| TimelockController | `CBBI5GUNLMY5X7M7DQEKMZ3SODYCMXUIVRIXE6UBY4B7W7S3YDGTNVHO` |

- **Guest `image_id`:** `b1f63151fccf1d509a1722eec40e7e987a752975d4471a393ab8d9e65108439c`
- **Verifier selector:** `73c457ba`
- **Live `submit` tx (proof verified + solvency recorded):**
  [`8ccabdf7…`](https://stellar.expert/explorer/testnet/tx/8ccabdf7124cbb3dfb9c73dd529697a8a9efb8ce7b5b7596b8fe3febd019824f)

The recorded on-chain attestation:

```json
{ "solvent": true, "ratio_bps": 12272, "n_accounts": 3,
  "statement_ts": 1750000000, "journal_digest": "1b9efb5c…427a", "ledger": 3286724 }
```

`ratio_bps: 12272` = **122.72%** collateralization (assets 1,350,000 / liabilities 1,100,000),
proven without revealing either total.

## Repo layout

```
por/                          RISC Zero zkVM project
  methods/guest/src/main.rs   the guest: sum, assert solvency, commit public journal
  host/src/main.rs            host: generates the Groth16 proof, writes proof.txt
contracts/proof-of-reserves/  our Soroban application contract
  src/lib.rs                  init / submit / latest — verifies via the router
scripts/run_demo.sh           end-to-end demo runner (build → prove → deploy → verify)
DEPLOYMENTS.md                all deployed addresses
```

## Run it yourself

Prereqs: Linux x86_64, Rust, RISC Zero (`rzup`), Stellar CLI, Docker (for Groth16),
and a clone of the Nethermind verifier at `./verifier`. See [`scripts/run_demo.sh`](scripts/run_demo.sh).

```bash
# 1. Generate a Groth16 proof of reserves (private books -> proof.txt)
cd por && cargo run -p host && cd ..

# 2. Verify the proof directly against the deployed router (simulation)
ROUTER=CDZG66I3HWXFNZW7BKAZH2C4R5BZMCFVBXNVF4IDNRHDL3XYNS44W2Y5
stellar contract invoke --send=no --network testnet --source issuer --id $ROUTER -- \
  verify --seal $(sed -n 1p por/proof.txt) \
         --image_id $(sed -n 2p por/proof.txt) \
         --journal $(sed -n 3p por/proof.txt)

# 3. Full app flow: submit to ProofOfReserves, which verifies + records solvency
POR=CCULU6FRJBTG77ZZNEXWYMKMYGEDLLS6746V2ZCZZWT2RM2RHWY7DUGA
stellar contract invoke --network testnet --source issuer --id $POR -- \
  submit --seal $(sed -n 1p por/proof.txt) \
         --image_id $(sed -n 2p por/proof.txt) \
         --journal $(cat por/journal.hex)
```

## Honest notes / limitations (work-in-progress)

- **Demo data.** The issuer's books in [`por/host/src/main.rs`](por/host/src/main.rs)
  are hardcoded demo values. In production these come from real account data.
- **Input-truth anchor.** A ZK proof proves the *computation* was correct on *some*
  input. Binding it to an issuer's *real, complete* books needs (a) public on-chain
  asset addresses anyone can sum, (b) a Merkle root of liabilities so each customer can
  verify their balance is included, and (c) signed attestations for off-chain reserves.
  We commit a single `sha256` liability commitment as a placeholder; extending it to a
  full Merkle tree for per-customer inclusion proofs is the clear next step.
- **Snapshot, not continuous.** Proof-of-reserves is a point-in-time snapshot; a real
  deployment runs frequent/randomized snapshots to resist window-dressing.
- **Not audited.** The Nethermind verifier and this contract are unaudited; testnet only,
  do not use with real assets.