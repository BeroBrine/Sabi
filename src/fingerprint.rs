use crate::fft::fft::FFTDistribution;
use ordered_float::OrderedFloat;
use std::collections::HashMap;

/// Tunable parameters
const MAX_TARGET_ZONE: usize = 60; // look ahead ~10s
const MIN_TARGET_ZONE_DIST: usize = 1;
const FREQ_STEP: f32 = 50.0; // coarser bins
const DELTA_STEP: f32 = 0.1; // 100ms bins

#[derive(Debug)]
pub struct FingerprintInfo {
    pub hash: u64,
    pub abs_anchor_tm_offset: f32,
    pub song_id: u32,
}

#[derive(Debug)]
pub struct VoteResult {
    pub song_id: u32,
    pub score: usize,
    pub time_offset: f32,
}

/// Quantize a frequency in Hz into coarse bins
fn quantize_freq(freq: f32) -> u32 {
    (freq / FREQ_STEP).round() as u32
}

/// Quantize a time delta into coarse bins
fn quantize_time_delta(delta: f32) -> u32 {
    ((delta / DELTA_STEP).round() as u32).min(16383)
}

/// Generate fingerprints with quantization + fan-out
pub fn generate_audio_fingerprint(fft_buffer: &Vec<FFTDistribution>) -> Vec<FingerprintInfo> {
    let buf_len = fft_buffer.len();
    let mut fingerprints = Vec::new();

    for (idx, fft_distribution) in fft_buffer.iter().enumerate() {
        let time = fft_distribution.time.into_inner();

        for anchor_peak in &fft_distribution.peaks {
            let anchor_freq_bin = quantize_freq(anchor_peak.freq.into_inner());

            // look ahead within target zone
            let start_idx = idx + MIN_TARGET_ZONE_DIST;
            let end_idx = (idx + MAX_TARGET_ZONE).min(buf_len);

            if start_idx >= end_idx {
                continue;
            }

            for slice in &fft_buffer[start_idx..end_idx] {
                let time_delta = slice.time.into_inner() - time;
                if time_delta <= 0.0 {
                    continue;
                }
                let delta_bin = quantize_time_delta(time_delta);

                for target_peak in &slice.peaks {
                    let target_freq_bin = quantize_freq(target_peak.freq.into_inner());

                    // construct 64-bit hash
                    let hash = (anchor_freq_bin as u64) << 30
                        | (target_freq_bin as u64) << 14
                        | (delta_bin as u64);

                    fingerprints.push(FingerprintInfo {
                        hash,
                        abs_anchor_tm_offset: time,
                        song_id: 1, // will be set properly when ingesting
                    });
                }
            }
        }
    }

    fingerprints
}

/// Vote using histogram of offsets (robust Shazam-like approach)
pub fn vote_best_matches(
    query_fingerprints: &[FingerprintInfo],
    db_matches_by_hash: &HashMap<u64, Vec<(u32, f32)>>,
    top_k: usize,
) -> Vec<VoteResult> {
    if query_fingerprints.is_empty() {
        return Vec::new();
    }

    // offset_histograms[song_id][offset_bin] = vote count
    let mut offset_histograms: HashMap<u32, HashMap<i32, usize>> = HashMap::new();

    for fp in query_fingerprints {
        if let Some(db_matches) = db_matches_by_hash.get(&fp.hash) {
            for &(song_id, db_time) in db_matches {
                let offset = db_time - fp.abs_anchor_tm_offset;
                let offset_bin = (offset / 0.020).round() as i32; // 50 ms bins

                *offset_histograms
                    .entry(song_id)
                    .or_default()
                    .entry(offset_bin)
                    .or_default() += 1;
            }
        }
    }

    // For each song, take the offset bin with max votes
    let mut results = Vec::new();
    for (song_id, hist) in offset_histograms {
        if let Some((&best_bin, &score)) = hist.iter().max_by_key(|&(_, &v)| v) {
            results.push(VoteResult {
                song_id,
                score,
                time_offset: best_bin as f32 * 0.020, // convert back to seconds
            });
        }
    }

    results.sort_by(|a, b| b.score.cmp(&a.score));
    if results.len() > top_k {
        results.truncate(top_k);
    }

    results
}
