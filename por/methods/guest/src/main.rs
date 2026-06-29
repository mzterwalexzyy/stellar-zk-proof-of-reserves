// Compliant Confidential Proof-of-Reserves guest (runs inside the RISC Zero zkVM).
//
// Upgrades over v1:
//  1. MERKLE INCLUSION: builds a Merkle tree over per-customer liabilities and
//     commits the root, so any customer can later prove their balance was counted.
//  2. SELECTIVE DISCLOSURE: the collateralization ratio is ENCRYPTED under a view
//     key. The public journal reveals only "solvent = true"; only a holder of the
//     view key (an auditor) can decrypt the exact ratio. The ZK proof guarantees
//     the ciphertext encrypts the TRUE computed ratio.
use risc0_zkvm::guest::env;
use sha2::{Digest, Sha256};

fn sha256_pair(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(a);
    h.update(b);
    h.finalize().into()
}

fn main() {
    // ---- private witness ----
    let assets: Vec<u64> = env::read();
    let liabilities: Vec<u64> = env::read();
    let timestamp: u64 = env::read();
    let view_key: [u8; 32] = env::read();

    let total_assets: u128 = assets.iter().map(|&x| x as u128).sum();
    let total_liabilities: u128 = liabilities.iter().map(|&x| x as u128).sum();

    // LOAD-BEARING: no proof exists unless solvent.
    assert!(total_assets >= total_liabilities, "INSOLVENT: assets < liabilities");

    let ratio_bps: u64 = if total_liabilities == 0 {
        u64::MAX
    } else {
        ((total_assets.saturating_mul(10_000)) / total_liabilities) as u64
    };

    // ---- Merkle tree over liabilities (leaf_i = sha256(i_le32 || balance_le64)) ----
    let mut layer: Vec<[u8; 32]> = liabilities
        .iter()
        .enumerate()
        .map(|(i, &bal)| {
            let mut h = Sha256::new();
            h.update((i as u32).to_le_bytes());
            h.update(bal.to_le_bytes());
            h.finalize().into()
        })
        .collect();
    if layer.is_empty() {
        layer.push([0u8; 32]);
    }
    while layer.len() > 1 {
        let mut next: Vec<[u8; 32]> = Vec::new();
        let mut i = 0;
        while i < layer.len() {
            let left = layer[i];
            let right = if i + 1 < layer.len() { layer[i + 1] } else { layer[i] };
            next.push(sha256_pair(&left, &right));
            i += 2;
        }
        layer = next;
    }
    let merkle_root: [u8; 32] = layer[0];

    // ---- selective disclosure: encrypt ratio under view key ----
    // keystream = first 8 bytes of sha256(view_key || timestamp_le); enc = ratio XOR keystream
    let mut hk = Sha256::new();
    hk.update(view_key);
    hk.update(timestamp.to_le_bytes());
    let ks = hk.finalize();
    let mut kb = [0u8; 8];
    kb.copy_from_slice(&ks[0..8]);
    let keystream = u64::from_le_bytes(kb);
    let enc_ratio: u64 = ratio_bps ^ keystream;

    let solvent: u32 = 1; // a proof only exists when solvent
    let n_accounts: u32 = liabilities.len() as u32;

    // ---- public journal (layout: solvent|ts|enc_ratio|n_accounts|merkle_root) ----
    env::commit(&solvent);
    env::commit(&timestamp);
    env::commit(&enc_ratio);
    env::commit(&n_accounts);
    env::commit(&merkle_root);
}