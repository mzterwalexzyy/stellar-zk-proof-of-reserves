#![no_std]
//! # Compliant Confidential Proof-of-Reserves (Soroban)
//!
//! Verifies a RISC Zero Groth16 proof that an issuer is solvent
//! (assets >= liabilities), and adds two compliance features:
//!
//!  * SELECTIVE DISCLOSURE: only `solvent` is public. The collateralization
//!    ratio is stored encrypted; an auditor holding the view key decrypts it
//!    off-chain. The proof guarantees the ciphertext encrypts the true ratio.
//!  * MERKLE INCLUSION: the proof commits a Merkle root over customer
//!    liabilities. `verify_inclusion` lets any customer prove on-chain that
//!    their balance was counted in the solvency total.
use risc0_interface::RiscZeroVerifierRouterClient;
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Bytes, BytesN, Env,
    Vec,
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
    /// Public: a record only exists if verification passed and solvent==1.
    pub solvent: bool,
    /// Confidential: ratio_bps XOR keystream. Auditor decrypts with view key.
    pub enc_ratio: u64,
    /// As-of timestamp of the books (also the encryption nonce).
    pub statement_ts: u64,
    /// Number of liability accounts in the Merkle tree.
    pub n_accounts: u32,
    /// Merkle root over customer liabilities (for inclusion proofs).
    pub merkle_root: BytesN<32>,
    /// sha256 of the verified public journal.
    pub journal_digest: BytesN<32>,
    pub ledger: u32,
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
    NotSolvent = 5,
    NoAttestation = 6,
}

#[contract]
pub struct ProofOfReserves;

#[contractimpl]
impl ProofOfReserves {
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

    /// Submit a Groth16 proof of reserves. Verifies on-chain, then records the
    /// attestation. Public sees only solvency; the ratio stays encrypted.
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

        if image_id != cfg.image_id {
            return Err(Error::UnexpectedImageId);
        }
        // journal layout: solvent(4) | ts(8) | enc_ratio(8) | n_accounts(4) | merkle_root(32 words)
        if journal.len() < 152 {
            return Err(Error::BadJournalLength);
        }

        let journal_digest: BytesN<32> = env.crypto().sha256(&journal).into();

        // verify on-chain (reverts if invalid)
        let router = RiscZeroVerifierRouterClient::new(&env, &cfg.router);
        router.verify(&seal, &image_id, &journal_digest);

        let solvent_flag = read_u32_le(&journal, 0);
        if solvent_flag != 1 {
            return Err(Error::NotSolvent);
        }
        let statement_ts = read_u64_le(&journal, 4);
        let enc_ratio = read_u64_le(&journal, 12);
        let n_accounts = read_u32_le(&journal, 20);
        let merkle_root = read_root_words(&env, &journal, 24);

        let rec = ProofRecord {
            solvent: true,
            enc_ratio,
            statement_ts,
            n_accounts,
            merkle_root,
            journal_digest,
            ledger: env.ledger().sequence(),
            recorded_at: env.ledger().timestamp(),
        };
        env.storage().instance().set(&DataKey::Latest, &rec);
        let count: u32 = env.storage().instance().get(&DataKey::Count).unwrap_or(0);
        env.storage().instance().set(&DataKey::Count, &(count + 1));

        // public event: solvency only (ratio stays confidential)
        env.events().publish(
            (symbol_short!("solvent"),),
            (rec.statement_ts, rec.n_accounts),
        );
        Ok(rec)
    }

    /// Prove on-chain that `leaf` (a customer's balance commitment) is included
    /// in the latest attested Merkle root. `path` is the sibling hashes from
    /// leaf to root; `index` is the customer's position.
    pub fn verify_inclusion(
        env: Env,
        leaf: BytesN<32>,
        index: u32,
        path: Vec<BytesN<32>>,
    ) -> Result<bool, Error> {
        let rec: ProofRecord = env
            .storage()
            .instance()
            .get(&DataKey::Latest)
            .ok_or(Error::NoAttestation)?;

        let mut cur = leaf;
        let mut idx = index;
        for sib in path.iter() {
            cur = if idx & 1 == 0 {
                hash_pair(&env, &cur, &sib)
            } else {
                hash_pair(&env, &sib, &cur)
            };
            idx >>= 1;
        }
        Ok(cur == rec.merkle_root)
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

fn hash_pair(env: &Env, a: &BytesN<32>, b: &BytesN<32>) -> BytesN<32> {
    let mut d = Bytes::new(env);
    d.append(&a.clone().into());
    d.append(&b.clone().into());
    env.crypto().sha256(&d).into()
}

// RISC Zero serializes [u8;32] as 32 little-endian u32 words, so each root byte
// sits at offset off + i*4.
fn read_root_words(env: &Env, b: &Bytes, off: u32) -> BytesN<32> {
    let mut arr = [0u8; 32];
    let mut i = 0u32;
    while i < 32 {
        arr[i as usize] = b.get(off + i * 4).unwrap_or(0);
        i += 1;
    }
    BytesN::from_array(env, &arr)
}

fn read_u64_le(b: &Bytes, off: u32) -> u64 {
    let mut v: u64 = 0;
    let mut i: u32 = 0;
    while i < 8 {
        v |= (b.get(off + i).unwrap_or(0) as u64) << (8 * i);
        i += 1;
    }
    v
}

fn read_u32_le(b: &Bytes, off: u32) -> u32 {
    let mut v: u32 = 0;
    let mut i: u32 = 0;
    while i < 4 {
        v |= (b.get(off + i).unwrap_or(0) as u32) << (8 * i);
        i += 1;
    }
    v
}