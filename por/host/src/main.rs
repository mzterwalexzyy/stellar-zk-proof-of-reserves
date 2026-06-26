// Host: generates a Groth16 proof of reserves and writes proof.txt for the
// on-chain Stellar verifier (seal / image_id / journal_digest).
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
    let assets: Vec<u64> = vec![400_000, 350_000, 600_000]; // total 1,350,000
    let liabilities: Vec<u64> = vec![200_000, 500_000, 400_000]; // total 1,100,000
    let timestamp: u64 = 1_750_000_000;

    let env = ExecutorEnv::builder()
        .write(&assets).unwrap()
        .write(&liabilities).unwrap()
        .write(&timestamp).unwrap()
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

    let (ratio_bps, ts, n_accounts, commitment): (u64, u64, u32, [u8; 32]) =
        receipt.journal.decode().unwrap();

    println!("   solvent:              TRUE (a valid proof exists)");
    println!("   ratio_bps:            {} ({:.2}%)", ratio_bps, ratio_bps as f64 / 100.0);
    println!("   timestamp:            {}", ts);
    println!("   accounts:             {}", n_accounts);
    println!("   liability_commitment: {}", hex::encode(commitment));

    let out = format!(
        "{}\n{}\n{}\n",
        hex::encode(&seal),
        hex::encode(image_id.as_bytes()),
        hex::encode(journal_digest)
    );
    fs::write("proof.txt", &out).unwrap();
    fs::write("journal.hex", hex::encode(&journal_bytes)).unwrap();
    println!(">> wrote proof.txt (seal / image_id / journal_digest) and journal.hex");
}