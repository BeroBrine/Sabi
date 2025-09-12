mod audio_processor;
mod fft;

use crate::{audio_processor::audio_processor::AudioProcessor, fft::fft::CooleyTukeyFFT};

fn main() {
    let audio_processor = AudioProcessor::new();
    let fft = CooleyTukeyFFT::default();

    let (audio_samples, sample_rate) = audio_processor.get_decoded_audio();
    let fingerprints = fft.fingerprint_audio(audio_samples, sample_rate);

    let mut i = 0;
    while i < 10 {
        let a = fingerprints.get(i).unwrap();
        println!(
            "For time {} the peak frequency and magnitude is {:?} ",
            a.0, a.1
        );
        i += 1;
    }
}
