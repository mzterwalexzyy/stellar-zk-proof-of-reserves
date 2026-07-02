// Host: generates a Groth16 proof for the compliant confidential PoR guest.
// Books can be overridden via env vars ASSETS / LIABILITIES (comma-separated)
// to demo both the solvent and the insolvent (no-proof-possible) scenarios.
use methods::{POR_GUEST_ELF, POR_GUEST_ID};
use risc0_ethereum_contracts::encode_seal;
use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts};
use sha2::{Digest, Sha256};
use std::fs;

fn parse_vec(name: &str, default: &[u64]) -> Vec<u64> {
    match std::env::var(name) {
        Ok(s) => s
            .split(',')
            .map(|x| x.trim())
            .filter(|x| !x.is_empty())
            .map(|x| x.parse::<u64>().expect("values must be integers"))
            .collect(),
        Err(_) => default.to_vec(),
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
        .init();

    let assets = parse_vec("ASSETS", &[400_000, 350_000, 600_000]);
    let liabilities = parse_vec("LIABILITIES", &[200_000, 500_000, 400_000]);
    let timestamp: u64 = 1_750_000_000;
    let view_key: [u8; 32] = *b"audit-view-key-demo-0123456789!!";

    let ta: u128 = assets.iter().map(|&x| x as u128).sum();
    let tl: u128 = liabilities.iter().map(|&x| x as u128).sum();
    println!(">> Books: assets = {ta}, liabilities = {tl}");
    println!(">> Proving solvency with Groth16 (zkVM + STARK->SNARK wrap)...");

    let env = ExecutorEnv::builder()
        .write(&assets).unwrap()
        .write(&liabilities).unwrap()
        .write(&timestamp).unwrap()
        .write(&view_key).unwrap()
        .build().unwrap();

    let prover = default_prover();
    let opts = ProverOpts::groth16();

    // The guest asserts assets >= liabilities. If it doesn't hold, the guest
    // panics during execution and NO proof can be produced.
    let receipt = match prover.prove_with_opts(env, POR_GUEST_ELF, &opts) {
        Ok(info) => info.receipt,
        Err(e) => {
            eprintln!("\n==================================================================");
            eprintln!("  X  PROOF GENERATION FAILED -- ISSUER IS INSOLVENT");
            eprintln!("==================================================================");
            eprintln!("  assets ({ta}) < liabilities ({tl})");
            eprintln!("  The guest asserted assets >= liabilities and it did NOT hold,");
            eprintln!("  so NO valid proof exists. There is nothing to submit to Stellar.");
            eprintln!("  (prover error: {e})");
            eprintln!("==================================================================");
            std::process::exit(1);
        }
    };
    receipt.verify(POR_GUEST_ID).unwrap();

    let seal = encode_seal(&receipt).unwrap();
    let journal_bytes = receipt.journal.bytes.clone();
    let journal_digest: [u8; 32] = Sha256::digest(&journal_bytes).into();
    let image_id = risc0_zkvm::sha::Digest::from(POR_GUEST_ID);

    let (solvent, ts, enc_ratio, n_accounts, merkle_root): (u32, u64, u64, u32, [u8; 32]) =
        receipt.journal.decode().unwrap();

    println!("\n--- PUBLIC sees (on-chain, no view key) ---");
    println!("  solvent:      {}", solvent == 1);
    println!("  ratio:        <ENCRYPTED>  (ciphertext {:#018x})", enc_ratio);
    println!("  merkle_root:  {}", hex::encode(merkle_root));
    println!("  accounts:     {}", n_accounts);

    let mut hk = Sha256::new();
    hk.update(view_key);
    hk.update(ts.to_le_bytes());
    let ks = hk.finalize();
    let mut kb = [0u8; 8];
    kb.copy_from_slice(&ks[0..8]);
    let ratio = enc_ratio ^ u64::from_le_bytes(kb);
    println!("\n--- AUDITOR (with view key) decrypts ---");
    println!("  ratio_bps:    {} ({:.2}%)", ratio, ratio as f64 / 100.0);

    let out = format!(
        "{}\n{}\n{}\n",
        hex::encode(&seal),
        hex::encode(image_id.as_bytes()),
        hex::encode(journal_digest)
    );
    fs::write("proof.txt", &out).unwrap();
    fs::write("journal.hex", hex::encode(&journal_bytes)).unwrap();
    fs::write("viewkey.hex", hex::encode(view_key)).unwrap();
    println!("\n>> wrote proof.txt, journal.hex, viewkey.hex");
}