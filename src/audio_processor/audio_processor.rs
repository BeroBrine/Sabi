use std::env;
use std::fs::File;

use symphonia::core::audio::{AudioBuffer, Channels, Signal, SignalSpec};
use symphonia::core::codecs::{CodecRegistry, DecoderOptions};
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

    pub fn get_decoded_audio(&self) -> Vec<f32> {
        let mut decoded_audio_samples = Vec::<f32>::new();

        self.generate_audio_samples(&mut decoded_audio_samples);

        decoded_audio_samples
    }

    fn generate_audio_samples(&self, decoded_audio_samples: &mut Vec<f32>) {
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
        let decoder_options = DecoderOptions::default();

        println!("the decoded type is {} ", codec_params.codec);
        let mut decoder = self
            .codec_registry
            .make(codec_params, &decoder_options)
            .unwrap();

        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(_) => break,
            };

            let decoded_packet = decoder.decode(&packet).unwrap();

            let mut stereo_buffer =
                AudioBuffer::<f32>::new(decoded_packet.capacity() as u64, *decoded_packet.spec());

            // fills the audio buffer with the packet.
            decoded_packet.convert(&mut stereo_buffer);

            let mono_buffer = self.convert_stereo_to_mono(stereo_buffer);

            decoded_audio_samples.push(mono_buffer.frames() as f32);
        }
    }

    fn convert_stereo_to_mono(&self, stereo_buffer: AudioBuffer<f32>) -> AudioBuffer<f32> {
        if stereo_buffer.spec().channels.count() < 2 {
            panic!("The buffer must not be mono to convert");
        }

        let left_channel = stereo_buffer.chan(0);
        let right_channel = stereo_buffer.chan(1);

        //INFO: i will implement the resampling algorithm myself (🤡).

        let mono_spec = SignalSpec::new(stereo_buffer.spec().rate, Channels::FRONT_LEFT);

        let mut mono_buffer = AudioBuffer::<f32>::new(stereo_buffer.capacity() as u64, mono_spec);
        mono_buffer.render_reserved(Some(stereo_buffer.frames()));

        let mono_plane = mono_buffer.chan_mut(0);

        for i in 0..stereo_buffer.frames() {
            let left_audio = left_channel[i];
            let right_audio = right_channel[i];

            let averaged_mono_audio = (left_audio + right_audio) * 0.5;

            mono_plane[i] = averaged_mono_audio;
        }

        println!("the mono buffer is {:?}", mono_buffer.chan(0));

        mono_buffer
    }

    fn read_return_file(&self) -> File {
        let args: Vec<String> = env::args().collect();
        let file_path = args.get(1).unwrap();

        let file = File::open(file_path).unwrap();
        println!("read the file");
        file
    }
}
