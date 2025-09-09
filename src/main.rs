mod audio_processor;

use crate::audio_processor::audio_processor::AudioProcessor;

fn main() {
    let audio_processor = AudioProcessor::new();

    let audio_samples = audio_processor.get_decoded_audio();

    println!("{:?}", audio_samples);
}
