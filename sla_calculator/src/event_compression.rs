#![no_std]

use soroban_sdk::{symbol_short, Env, Symbol};

use crate::{SLAResult, SLAError, HISTORY_KEY};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const COMPRESSED_EVENTS_KEY: Symbol = symbol_short!("CEVT");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Compressed event representation for storage efficiency.
#[soroban_sdk::contracttype]
pub struct CompressedEvent {
    /// Packed fields: severity (2 bits) + status (1 bit) + rating (2 bits) + payment_type (1 bit)
    pub packed_flags: u32,
    /// Outage ID (compact).
    pub outage_id: Symbol,
    /// MTTR in minutes.
    pub mttr_minutes: u16,
    /// Threshold in minutes.
    pub threshold_minutes: u16,
    /// Amount (compressed i128).
    pub amount: i64,
    /// Config version hash (lower 32 bits).
    pub config_hash_lo: u32,
    /// Recorded at timestamp.
    pub recorded_at: u32,
}

/// Decompressed event for consumption.
#[soroban_sdk::contracttype]
pub struct DecompressedEvent {
    pub outage_id: Symbol,
    pub status: Symbol,
    pub mttr_minutes: u32,
    pub threshold_minutes: u32,
    pub amount: i128,
    pub payment_type: Symbol,
    pub rating: Symbol,
    pub config_version_hash: u64,
    pub recorded_at: u64,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Severity to numeric mapping for compression.
fn severity_to_bits(severity: &Symbol) -> u32 {
    if *severity == symbol_short!("critical") {
        0
    } else if *severity == symbol_short!("high") {
        1
    } else if *severity == symbol_short!("medium") {
        2
    } else {
        3 // low
    }
}

/// Status to numeric mapping.
fn status_to_bits(status: &Symbol) -> u32 {
    if *status == symbol_short!("met") {
        0
    } else {
        1 // viol
    }
}

/// Rating to numeric mapping.
fn rating_to_bits(rating: &Symbol) -> u32 {
    if *rating == symbol_short!("top") {
        0
    } else if *rating == symbol_short!("excel") {
        1
    } else if *rating == symbol_short!("good") {
        2
    } else {
        3 // poor
    }
}

/// Payment type to numeric mapping.
fn payment_to_bits(payment: &Symbol) -> u32 {
    if *payment == symbol_short!("rew") {
        0
    } else {
        1 // pen
    }
}

/// Numeric back to status symbol.
fn bits_to_status(bits: u32) -> Symbol {
    if bits == 0 {
        symbol_short!("met")
    } else {
        symbol_short!("viol")
    }
}

/// Numeric back to payment type symbol.
fn bits_to_payment(bits: u32) -> Symbol {
    if bits == 0 {
        symbol_short!("rew")
    } else {
        symbol_short!("pen")
    }
}

/// Numeric back to rating symbol.
fn bits_to_rating(bits: u32) -> Symbol {
    match bits {
        0 => symbol_short!("top"),
        1 => symbol_short!("excel"),
        2 => symbol_short!("good"),
        _ => symbol_short!("poor"),
    }
}

/// Compress an SLAResult into a CompressedEvent.
///
/// Packs multiple fields into fewer storage slots for gas efficiency.
pub fn compress_event(result: &SLAResult) -> CompressedEvent {
    let severity_bits = 0; // Severity not stored in result, use 0
    let status_bits = status_to_bits(&result.status);
    let rating_bits = rating_to_bits(&result.rating);
    let payment_bits = payment_to_bits(&result.payment_type);

    let packed_flags = (severity_bits << 4)
        | (status_bits << 3)
        | (rating_bits << 1)
        | payment_bits;

    CompressedEvent {
        packed_flags,
        outage_id: result.outage_id.clone(),
        mttr_minutes: result.mttr_minutes as u16,
        threshold_minutes: result.threshold_minutes as u16,
        amount: result.amount as i64,
        config_hash_lo: (result.config_version_hash & 0xFFFFFFFF) as u32,
        recorded_at: result.recorded_at as u32,
    }
}

/// Decompress a CompressedEvent back to SLAResult.
pub fn decompress_event(compressed: &CompressedEvent) -> SLAResult {
    let status_bits = (compressed.packed_flags >> 3) & 0x1;
    let rating_bits = (compressed.packed_flags >> 1) & 0x3;
    let payment_bits = compressed.packed_flags & 0x1;

    SLAResult {
        outage_id: compressed.outage_id.clone(),
        status: bits_to_status(status_bits),
        mttr_minutes: compressed.mttr_minutes as u32,
        threshold_minutes: compressed.threshold_minutes as u32,
        amount: compressed.amount as i128,
        payment_type: bits_to_payment(payment_bits),
        rating: bits_to_rating(rating_bits),
        config_version_hash: compressed.config_hash_lo as u64,
        recorded_at: compressed.recorded_at as u64,
    }
}

/// Compress a batch of SLAResults.
pub fn compress_batch(
    env: &Env,
    results: &soroban_sdk::Vec<SLAResult>,
) -> soroban_sdk::Vec<CompressedEvent> {
    let mut compressed = soroban_sdk::Vec::new(env);
    for i in 0..results.len() {
        compressed.push_back(compress_event(&results.get(i).unwrap()));
    }
    compressed
}

/// Decompress a batch of CompressedEvents.
pub fn decompress_batch(
    env: &Env,
    events: &soroban_sdk::Vec<CompressedEvent>,
) -> soroban_sdk::Vec<SLAResult> {
    let mut decompressed = soroban_sdk::Vec::new(env);
    for i in 0..events.len() {
        decompressed.push_back(decompress_event(&events.get(i).unwrap()));
    }
    decompressed
}

/// Store compressed events in contract storage.
pub fn store_compressed_events(
    env: &Env,
    events: &soroban_sdk::Vec<CompressedEvent>,
) {
    env.storage().instance().set(&COMPRESSED_EVENTS_KEY, events);
}

/// Retrieve compressed events from storage.
pub fn get_compressed_events(
    env: &Env,
) -> soroban_sdk::Vec<CompressedEvent> {
    env.storage()
        .instance()
        .get(&COMPRESSED_EVENTS_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env))
}

/// Calculate storage savings (approximate bytes saved).
pub fn estimate_savings(event_count: u32) -> u32 {
    // Original: ~128 bytes per event (9 fields, symbols are 32 bytes each)
    // Compressed: ~32 bytes per event (packed flags, compact fields)
    // Savings: ~96 bytes per event
    event_count * 96
}
