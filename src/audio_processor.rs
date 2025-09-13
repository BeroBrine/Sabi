use std::env;
use std::fs::File;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CodecRegistry, DecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::{Hint, Probe};
use symphonia::default;

pub struct AudioProcessor {
    codec_registry: &'static CodecRegistry,
    format_options: FormatOptions,
    metadata_options: MetadataOptions,
    probe: &'static Probe,
}

impl AudioProcessor {
    pub fn new() -> Self {
        Self {
            codec_registry: default::get_codecs(),
            format_options: FormatOptions::default(),
            metadata_options: MetadataOptions::default(),
            probe: symphonia::default::get_probe(),
        }
    }

    pub fn get_decoded_audio(&self) -> (Vec<f32>, u32) {
        let (decoded_audio_samples, sample_rate) = match self.generate_audio_samples() {
            Ok(k) => k,
            Err(e) => {
                panic!("Generating audio samples failed \n {}", e);
            }
        };

        (decoded_audio_samples, sample_rate)
    }

    fn generate_audio_samples(&self) -> Result<(Vec<f32>, u32), Box<dyn std::error::Error>> {
        let file = self.read_return_file();

        let source: Box<dyn MediaSource> = Box::new(file);

        let track = MediaSourceStream::new(source, Default::default());

        let prober = self
            .probe
            .format(
                &Hint::new(),
                track,
                &self.format_options,
                &self.metadata_options,
            )
            .expect("an error has occurred while probing");
        let mut format = prober.format;

        println!("{:?}", format.tracks());
        let codec_params = &format.tracks().get(0).unwrap().codec_params;
        let sample_rate = codec_params.sample_rate.unwrap();
        let decoder_options = DecoderOptions::default();

        println!("the decoded type is {} ", codec_params.codec);
        let mut decoder = self
            .codec_registry
            .make(codec_params, &decoder_options)
            .unwrap();

        let mut decoded_audio_samples = Vec::new();
        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                // EOF
                Err(Error::IoError(_)) => {
                    break;
                }
                Err(e) => return Err(Box::new(e)),
            };

            let decoded_packet = decoder.decode(&packet).unwrap();
            let num_channels = decoded_packet.spec().channels.count();

            let mut sample_buf =
                SampleBuffer::<f32>::new(decoded_packet.capacity() as u64, *decoded_packet.spec());
            sample_buf.copy_interleaved_ref(decoded_packet);

            // --- THE FIX IS HERE ---
            // Instead of pushing the number of frames, we extend the vector with the actual samples.
            for i in (0..sample_buf.len()).step_by(num_channels) {
                let frame = &sample_buf.samples()[i..i + num_channels];
                let mono_sample = frame.iter().sum::<f32>() / num_channels as f32;
                decoded_audio_samples.push(mono_sample);
            }
        }

        Ok((decoded_audio_samples, sample_rate))
    }

    fn read_return_file(&self) -> File {
        let args: Vec<String> = env::args().collect();
        let file_path = args.get(1).unwrap();

        let file = File::open(file_path).unwrap();
        println!("read the file");
        file
    }
}
