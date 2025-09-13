mod audio_processor;
mod fft;
mod fingerprint;
mod visualization;

use crate::fingerprint::generate_audio_fingerprint;
use crate::{audio_processor::AudioProcessor, fft::fft::CooleyTukeyFFT};

fn main() {
    let audio_processor = AudioProcessor::new();

    let fft = CooleyTukeyFFT::default();

    let (audio_samples, sample_rate) = audio_processor.get_decoded_audio();

    println!("the audio sample len is {} ", audio_samples.len());

    let fft_distribution = fft.generate_freq_time_distribution(audio_samples, sample_rate);

    generate_audio_fingerprint(&fft_distribution);

    // let song_name = env::args()
    //     .nth(1)
    //     .unwrap_or_else(|| String::from("Unknown Song"));
    //
    // if let Err(e) = write_heatmap_svg(&digested_audio, "heatmap.svg", &song_name) {
    //     eprintln!("Failed to write heatmap.svg: {}", e);
    // } else {
    //     println!("Wrote heatmap.svg");
    // }
}
