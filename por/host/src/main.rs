// Host: generates a Groth16 proof for the compliant confidential PoR guest and
// shows the PUBLIC view (solvent only) vs the AUDITOR view (decrypted ratio).
use methods::{POR_GUEST_ELF, POR_GUEST_ID};
use risc0_ethereum_contracts::encode_seal;
use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts};
use sha2::{Digest, Sha256};
use std::fs;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
        .init();

    // ---- DEMO confidential books (private to the issuer) ----
    let assets: Vec<u64> = vec![400_000, 350_000, 600_000]; // 1,350,000
    let liabilities: Vec<u64> = vec![200_000, 500_000, 400_000]; // 1,100,000
    let timestamp: u64 = 1_750_000_000;
    // Demo view key — in production shared out-of-band with the auditor only.
    let view_key: [u8; 32] = *b"audit-view-key-demo-0123456789!!";

    let env = ExecutorEnv::builder()
        .write(&assets).unwrap()
        .write(&liabilities).unwrap()
        .write(&timestamp).unwrap()
        .write(&view_key).unwrap()
        .build().unwrap();

    println!(">> Proving solvency with Groth16 (zkVM + STARK->SNARK wrap)...");
    let prover = default_prover();
    let opts = ProverOpts::groth16();
    let receipt = prover.prove_with_opts(env, POR_GUEST_ELF, &opts).unwrap().receipt;
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

    // Auditor decrypts with the view key.
    let mut hk = Sha256::new();
    hk.update(view_key);
    hk.update(ts.to_le_bytes());
    let ks = hk.finalize();
    let mut kb = [0u8; 8];
    kb.copy_from_slice(&ks[0..8]);
    let keystream = u64::from_le_bytes(kb);
    let ratio = enc_ratio ^ keystream;
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