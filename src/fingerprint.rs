use crate::fft::fft::FFTDistribution;
use ordered_float::OrderedFloat;
use std::hash::{DefaultHasher, Hash, Hasher};

const MAX_TARGET_ZONE: usize = 10;

#[derive(Hash)]
struct Hashable {
    freq_1: OrderedFloat<f32>,
    freq_2: OrderedFloat<f32>,
    time_delta: OrderedFloat<f32>,
}

pub fn generate_audio_fingerprint(fft_buffer: &Vec<FFTDistribution>) {
    let buf_len = fft_buffer.len();
    for (idx, fft_distribution) in fft_buffer.iter().enumerate() {
        let time = &fft_distribution.time.into_inner();

        let peaks = &fft_distribution.peaks;

        for peak in peaks {
            let anchor = peak;
            let anchor_freq = &anchor.freq.into_inner();

            if idx + MAX_TARGET_ZONE >= buf_len {
                break;
            }
            let next_buf_slice = &fft_buffer[idx + 1..idx + MAX_TARGET_ZONE];

            for slice in next_buf_slice {
                for peak in &slice.peaks {
                    let freq_2 = peak.freq.into_inner();
                    let time_delta = slice.time.into_inner() - time;
                    let mut s = DefaultHasher::new();

                    let info = Hashable {
                        freq_1: OrderedFloat(*anchor_freq),
                        freq_2: OrderedFloat(freq_2),
                        time_delta: OrderedFloat(time_delta),
                    };

                    info.hash(&mut s);
                    let hashed_value = s.finish();
                    println!(
                        "For anchor time {} , and slice time {} , generated hash is {} ",
                        time,
                        slice.time.into_inner(),
                        hashed_value
                    );
                }
            }
        }
    }
}
