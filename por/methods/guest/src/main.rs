// Proof-of-Reserves guest program (runs inside the RISC Zero zkVM).
//
// Private inputs (never revealed): per-account asset balances and liabilities.
// The SOLVENCY check is load-bearing: a valid proof can ONLY be produced when
// total_assets >= total_liabilities. The mere existence of a valid proof for
// this program's image ID is therefore a proof of solvency.
use risc0_zkvm::guest::env;
use sha2::{Digest, Sha256};

fn main() {
    // ---- private witness ----
    let assets: Vec<u64> = env::read();
    let liabilities: Vec<u64> = env::read();
    // ---- public-ish input (bound into the journal) ----
    let timestamp: u64 = env::read();

    let total_assets: u128 = assets.iter().map(|&x| x as u128).sum();
    let total_liabilities: u128 = liabilities.iter().map(|&x| x as u128).sum();

    // LOAD-BEARING: no proof exists unless the issuer is solvent.
    assert!(total_assets >= total_liabilities, "INSOLVENT: assets < liabilities");

    // Collateralization ratio in basis points (10000 = 100%).
    let ratio_bps: u64 = if total_liabilities == 0 {
        u64::MAX
    } else {
        ((total_assets.saturating_mul(10_000)) / total_liabilities) as u64
    };

    // Commitment to the liability set so customers can later verify inclusion.
    let mut hasher = Sha256::new();
    for l in &liabilities {
        hasher.update(l.to_le_bytes());
    }
    let liability_commitment: [u8; 32] = hasher.finalize().into();

    let n_accounts: u32 = liabilities.len() as u32;

    // ---- public journal (revealed) ----
    env::commit(&ratio_bps);
    env::commit(&timestamp);
    env::commit(&n_accounts);
    env::commit(&liability_commitment);
}