use crate::fft::fft::FFTDistribution; // Assuming this is your path
use ordered_float::OrderedFloat;
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
};
use twox_hash::XxHash3_64;

// --- TUNABLE PARAMETERS FOR BETTER ACCURACY ---
/// How many FFT chunks to look ahead for pairing peaks.
const MAX_TARGET_ZONE: usize = 5;
/// How many FFT chunks to *skip* after the anchor chunk to find a target peak.
/// This is the crucial change to create more distinctive hashes.
const MIN_TARGET_ZONE_DIST: usize = 1;

#[derive(Hash, Debug)]
struct Hashable {
    freq_1: OrderedFloat<f32>,
    freq_2: OrderedFloat<f32>,
    time_delta: OrderedFloat<f32>,
}

#[derive(Debug)]
pub struct FingerprintInfo {
    pub hash: u64,
    pub abs_anchor_tm_offset: f32,
    pub song_id: u32,
}

pub fn generate_audio_fingerprint(fft_buffer: &Vec<FFTDistribution>) -> Vec<FingerprintInfo> {
    let buf_len = fft_buffer.len();
    let mut fingerprints: Vec<FingerprintInfo> = Vec::new();

    const FREQ_PRECISION: f32 = 1.0;
    const TIME_PRECISION: f32 = 0.01;

    for (idx, fft_distribution) in fft_buffer.iter().enumerate() {
        let time = fft_distribution.time.into_inner();
        let anchor_peaks = &fft_distribution.peaks;

        for anchor_peak in anchor_peaks {
            let anchor_freq = anchor_peak.freq.into_inner();

            // --- REVISED LOGIC: Define the start and end of the target zone ---
            let start_idx = idx + MIN_TARGET_ZONE_DIST;
            let end_idx = (idx + MAX_TARGET_ZONE).min(buf_len); // Ensure we don't go out of bounds

            if start_idx >= end_idx {
                continue;
            }

            // Iterate through only the chunks within our defined target zone
            for slice in &fft_buffer[start_idx..end_idx] {
                for target_peak in &slice.peaks {
                    let freq_2 = target_peak.freq.into_inner();
                    let time_delta = slice.time.into_inner() - time;

                    // Use bit-packing exactly like Go version
                    let anchor_freq_int = anchor_freq as i32; // real part of complex frequency
                    let target_freq_int = freq_2 as i32; // real part of complex frequency
                    let delta_ms = ((time_delta * 1000.0) as u32).min(16383); // 14 bits max
                    
                    // Debug: Print some values
                    if fingerprints.len() < 3 {
                        println!("Fingerprint {}: anchor_freq={}, target_freq={}, delta_ms={}", 
                                fingerprints.len(), anchor_freq_int, target_freq_int, delta_ms);
                    }
                    
                    // Bit-packing exactly like Go: anchorFreq<<23 | targetFreq<<14 | deltaMs
                    let hashed_value = (anchor_freq_int as u32) << 23 | (target_freq_int as u32) << 14 | delta_ms;

                    let song_info = FingerprintInfo {
                        hash: hashed_value as u64,
                        abs_anchor_tm_offset: time,
                        song_id: 1,
                    };
                    fingerprints.push(song_info);
                }
            }
        }
    }
    fingerprints
}

#[derive(Debug)]
pub struct VoteResult {
    pub song_id: u32,
    pub score: usize,
    pub time_offset: f32, // Time offset in seconds where the snippet occurs in the song
}

// The voting algorithm remains unchanged. Its accuracy depends on the quality
// of the fingerprints, which we have now improved.
pub fn vote_best_matches(
    query_fingerprints: &[FingerprintInfo],
    db_matches_by_hash: &HashMap<u64, Vec<(u32, f32)>>,
    _delta_bucket_secs: f32, // Not used in this implementation
    top_k: usize,
) -> Vec<VoteResult> {
    if query_fingerprints.is_empty() {
        return Vec::new();
    }

    // Collect all matches: song_id -> [(query_time, db_time)]
    let mut matches: HashMap<u32, Vec<(f32, f32)>> = HashMap::new();
    
    for fp in query_fingerprints.iter() {
        if let Some(db_matches) = db_matches_by_hash.get(&fp.hash) {
            for &(song_id, db_time) in db_matches.iter() {
                matches.entry(song_id)
                    .or_insert_with(Vec::new)
                    .push((fp.abs_anchor_tm_offset, db_time));
            }
        }
    }

    // Match Go's analyzeRelativeTiming exactly - O(n²) pairwise comparison
    let mut scores: HashMap<u32, (f64, f32)> = HashMap::new(); // (score, time_offset)
    
    for (song_id, times) in matches.iter() {
        if times.is_empty() {
            scores.insert(*song_id, (0.0, 0.0));
            continue;
        }
        
        // Calculate time offset for display (simple average)
        let mut total_offset = 0.0;
        for (sample_time, db_time) in times.iter() {
            total_offset += db_time - sample_time;
        }
        let avg_time_offset = total_offset / times.len() as f32;
        
        // Match Go's analyzeRelativeTiming exactly
        let mut count = 0;
        for i in 0..times.len() {
            for j in (i + 1)..times.len() {
                let sample_diff = (times[i].0 - times[j].0).abs();
                let db_diff = (times[i].1 - times[j].1).abs();
                if (sample_diff - db_diff).abs() < 0.1 { // 100ms tolerance like Go
                    count += 1;
                }
            }
        }
        
        scores.insert(*song_id, (count as f64, avg_time_offset));
    }

    let mut scored: Vec<VoteResult> = scores
        .into_iter()
        .map(|(song_id, (score, time_offset))| VoteResult { 
            song_id, 
            score: score as usize,
            time_offset
        })
        .collect();

    scored.sort_by(|a, b| b.score.cmp(&a.score));
    if scored.len() > top_k {
        scored.truncate(top_k);
    }
    scored
}
