mod audio_processor;
mod fft;

use crate::{audio_processor::audio_processor::AudioProcessor, fft::fft::CooleyTukeyFFT};

fn main() {
    let audio_processor = AudioProcessor::new();
    let fft = CooleyTukeyFFT::default();

    let (audio_samples, sample_rate) = audio_processor.get_decoded_audio();

    println!("the audio sample len is {} ", audio_samples.len());

    let fingerprints = fft.fingerprint_audio(audio_samples, sample_rate);
}
