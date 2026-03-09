use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use rayon::prelude::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use serde::Serialize;
use std::time::Instant;

pub const BASE58_ALPHABET: &str = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

#[derive(Debug, Clone, Serialize)]
pub struct MatchResult {
    pub public_key: String,
    pub private_key: String,
    pub attempts: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgressUpdate {
    pub attempts: u64,
    pub rate: f64, // keys/sec
    pub elapsed: f64,
    pub found: usize,
    pub needed: usize,
}

#[derive(Debug, Clone)]
pub enum GrinderEvent {
    Progress(ProgressUpdate),
    Match(MatchResult),
    Done { total_attempts: u64, elapsed: f64, found: usize },
}

pub fn validate_prefix(prefix: &str) -> Result<(), String> {
    if prefix.is_empty() {
        return Err("Prefix cannot be empty".to_string());
    }
    if prefix.len() > 6 {
        return Err("Prefix length cannot exceed 6 characters".to_string());
    }

    for c in prefix.chars() {
        if !BASE58_ALPHABET.contains(c) {
            let suggestion = match c {
                '0' => "⚠ '0' is not in the Solana alphabet — try '1' or 'o'",
                'O' => "⚠ 'O' looks like zero — Solana uses 'o' (lowercase)",
                'I' => "⚠ 'I' looks like 1 — Solana uses 'i' (lowercase) or '1'",
                'l' => "⚠ 'l' (lowercase L) — Solana uses 'L' (uppercase)",
                _ => "Invalid base58 character.",
            };
            return Err(suggestion.to_string());
        }
    }
    Ok(())
}

pub fn get_difficulty_estimate(prefix_len: usize) -> (String, String) {
    match prefix_len {
        1 => ("Instant".to_string(), "🟢 Instant".to_string()),
        2 => ("< 1 second".to_string(), "🟢 Instant".to_string()),
        3 => ("< 5 seconds".to_string(), "🟢 Under 5 seconds".to_string()),
        4 => ("~1 minute".to_string(), "🟡 About 1 minute".to_string()),
        5 => ("~1 hour".to_string(), "🟠 About 1 hour".to_string()),
        _ => ("~2-3 days".to_string(), "🔴 May take days — only use on a fast machine".to_string()),
    }
}

pub fn encode_private_key(sk: &SigningKey) -> String {
    let mut bytes = [0u8; 64];
    bytes[..32].copy_from_slice(sk.to_bytes().as_slice());
    bytes[32..].copy_from_slice(sk.verifying_key().to_bytes().as_slice());
    bs58::encode(&bytes).into_string()
}

pub fn grind(
    prefix: String,
    count: usize,
    tx: Sender<GrinderEvent>,
    abort_signal: Arc<AtomicBool>,
) {
    let prefix_lower = prefix.to_lowercase();
    let attempts = Arc::new(AtomicU64::new(0));
    let start_time = Instant::now();
    let found_count = Arc::new(AtomicU64::new(0));

    // Progress reporting thread
    let attempts_clone = attempts.clone();
    let found_clone = found_count.clone();
    let tx_clone = tx.clone();
    let abort_clone = abort_signal.clone();
    
    std::thread::spawn(move || {
        let mut last_attempts = 0;
        let mut last_time = Instant::now();
        
        while !abort_clone.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_secs(2));
            if abort_clone.load(Ordering::Relaxed) {
                break;
            }
            let current_attempts = attempts_clone.load(Ordering::Relaxed);
            let current_found = found_clone.load(Ordering::Relaxed) as usize;
            
            let current_time = Instant::now();
            let elapsed_since_last = current_time.duration_since(last_time).as_secs_f64();
            let added_attempts = current_attempts.saturating_sub(last_attempts);
            let rate = added_attempts as f64 / elapsed_since_last;
            
            last_attempts = current_attempts;
            last_time = current_time;

            let elapsed_total = start_time.elapsed().as_secs_f64();

            let _ = tx_clone.send(GrinderEvent::Progress(ProgressUpdate {
                attempts: current_attempts,
                rate,
                elapsed: elapsed_total,
                found: current_found,
                needed: count,
            }));
            
            if current_found >= count {
                break;
            }
        }
    });

    let _ = rayon::iter::repeat(()).take_any_while(|_| {
        if abort_signal.load(Ordering::Relaxed) {
            return false;
        }

        let current_found = found_count.load(Ordering::Relaxed) as usize;
        if current_found >= count {
            return false;
        }

        let mut rng = OsRng;
        let sk = SigningKey::generate(&mut rng);
        let current_attempt = attempts.fetch_add(1, Ordering::Relaxed) + 1;
        
        let addr = bs58::encode(sk.verifying_key().to_bytes().as_slice()).into_string();
        
        if addr.to_lowercase().starts_with(&prefix_lower) {
            // Found a match!
            let pk = encode_private_key(&sk);
            
            // To prevent multiple threads from overshooting the count, we use an atomic fetch_add
            let prior_found = found_count.fetch_add(1, Ordering::Relaxed) as usize;
            if prior_found < count {
                let _ = tx.send(GrinderEvent::Match(MatchResult {
                    public_key: addr,
                    private_key: pk,
                    attempts: current_attempt,
                }));
            }
            
            if prior_found + 1 >= count {
                abort_signal.store(true, Ordering::Relaxed);
                return false;
            }
        }
        true
    }).count();

    abort_signal.store(true, Ordering::Relaxed);
    
    let total_attempts = attempts.load(Ordering::Relaxed);
    let final_found = found_count.load(Ordering::Relaxed) as usize;
    let _ = tx.send(GrinderEvent::Done {
        total_attempts,
        elapsed: start_time.elapsed().as_secs_f64(),
        found: final_found,
    });
}
