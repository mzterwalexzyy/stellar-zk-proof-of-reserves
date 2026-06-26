#![no_std]
//! # Proof-of-Reserves (Soroban)
//!
//! Verifies a RISC Zero Groth16 proof on-chain that an issuer's total assets
//! are >= its total liabilities, WITHOUT revealing any individual balance.
//!
//! Security model: the guest program asserts `assets >= liabilities`, so a
//! valid proof for the configured `image_id` can only exist if the issuer is
//! solvent. This contract therefore (1) checks the proof is for OUR program,
//! (2) binds the passed public journal to the proof via its sha256 digest, and
//! (3) verifies the seal through the RISC Zero verifier router. If all pass,
//! solvency is proven and recorded on-chain.
use risc0_interface::RiscZeroVerifierRouterClient;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, BytesN, Env,
};

#[contracttype]
pub enum DataKey {
    Config,
    Latest,
    Count,
}

#[contracttype]
#[derive(Clone)]
pub struct Config {
    pub admin: Address,
    pub router: Address,
    pub image_id: BytesN<32>,
}

#[contracttype]
#[derive(Clone)]
pub struct ProofRecord {
    /// Always true when stored: a record only exists if verification passed.
    pub solvent: bool,
    /// Collateralization ratio in basis points (10000 = 100%).
    pub ratio_bps: u64,
    /// As-of timestamp of the issuer's books (from the proven journal).
    pub statement_ts: u64,
    /// Number of liability accounts covered by the proof.
    pub n_accounts: u32,
    /// sha256 of the public journal bound to the verified proof.
    pub journal_digest: BytesN<32>,
    /// Ledger sequence when this attestation was recorded.
    pub ledger: u32,
    /// On-chain timestamp when recorded.
    pub recorded_at: u64,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    UnexpectedImageId = 3,
    BadJournalLength = 4,
}

#[contract]
pub struct ProofOfReserves;

#[contractimpl]
impl ProofOfReserves {
    /// One-time setup: bind this contract to a verifier `router` and the
    /// `image_id` of the proof-of-reserves guest program it will accept.
    pub fn init(
        env: Env,
        admin: Address,
        router: Address,
        image_id: BytesN<32>,
    ) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Config) {
            return Err(Error::AlreadyInitialized);
        }
        env.storage()
            .instance()
            .set(&DataKey::Config, &Config { admin, router, image_id });
        env.storage().instance().set(&DataKey::Count, &0u32);
        Ok(())
    }

    /// Submit a Groth16 proof of reserves. Reverts unless the proof is valid
    /// for the configured guest program. Records solvency on success.
    pub fn submit(
        env: Env,
        seal: Bytes,
        image_id: BytesN<32>,
        journal: Bytes,
    ) -> Result<ProofRecord, Error> {
        let cfg: Config = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(Error::NotInitialized)?;

        // 1) the proof must be for OUR proof-of-reserves program.
        if image_id != cfg.image_id {
            return Err(Error::UnexpectedImageId);
        }

        // 2) bind the public journal to the proof.
        let journal_digest: BytesN<32> = env.crypto().sha256(&journal).into();

        // 3) verify the seal on-chain (reverts if invalid).
        let router = RiscZeroVerifierRouterClient::new(&env, &cfg.router);
        router.verify(&seal, &image_id, &journal_digest);

        // ---- proof is valid => issuer is solvent. Parse public fields. ----
        if journal.len() < 20 {
            return Err(Error::BadJournalLength);
        }
        let ratio_bps = read_u64_le(&journal, 0);
        let statement_ts = read_u64_le(&journal, 8);
        let n_accounts = read_u32_le(&journal, 16);

        let rec = ProofRecord {
            solvent: true,
            ratio_bps,
            statement_ts,
            n_accounts,
            journal_digest,
            ledger: env.ledger().sequence(),
            recorded_at: env.ledger().timestamp(),
        };
        env.storage().instance().set(&DataKey::Latest, &rec);
        let count: u32 = env.storage().instance().get(&DataKey::Count).unwrap_or(0);
        env.storage().instance().set(&DataKey::Count, &(count + 1));

        env.events().publish(
            (symbol_short!("solvent"),),
            (rec.ratio_bps, rec.statement_ts, rec.n_accounts),
        );
        Ok(rec)
    }

    pub fn latest(env: Env) -> Option<ProofRecord> {
        env.storage().instance().get(&DataKey::Latest)
    }

    pub fn count(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::Count).unwrap_or(0)
    }

    pub fn config(env: Env) -> Option<Config> {
        env.storage().instance().get(&DataKey::Config)
    }
}

fn read_u64_le(b: &Bytes, off: u32) -> u64 {
    let mut v: u64 = 0;
    let mut i: u32 = 0;
    while i < 8 {
        let byte = b.get(off + i).unwrap_or(0) as u64;
        v |= byte << (8 * i);
        i += 1;
    }
    v
}

fn read_u32_le(b: &Bytes, off: u32) -> u32 {
    let mut v: u32 = 0;
    let mut i: u32 = 0;
    while i < 4 {
        let byte = b.get(off + i).unwrap_or(0) as u32;
        v |= byte << (8 * i);
        i += 1;
    }
    v
}