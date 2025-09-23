use std::f32::consts::PI;
use std::fs::File;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use std::{env, thread};

use cpal::{Devices, SampleRate, StreamConfig, SupportedStreamConfig};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CodecRegistry, DecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::{Hint, Probe};
use symphonia::default;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub struct AudioProcessor {
    codec_registry: &'static CodecRegistry,
    format_options: FormatOptions,
    metadata_options: MetadataOptions,
    probe: &'static Probe,
}

impl AudioProcessor {
    pub const TARGET_SAMPLE_RATE: u32 = 11025;

    pub fn new() -> Self {
        Self {
            codec_registry: default::get_codecs(),
            format_options: FormatOptions::default(),
            metadata_options: MetadataOptions::default(),
            probe: symphonia::default::get_probe(),
        }
    }

    pub fn get_decoded_audio(&self, file_name: String) -> (Vec<f32>, u32) {
        let file = self.read_return_file(file_name);
        let (decoded_audio_samples, sample_rate) = match self.generate_audio_samples(file) {
            Ok(k) => k,
            Err(e) => {
                panic!("Generating audio samples failed \n {}", e);
            }
        };

        (decoded_audio_samples, sample_rate)
    }

    fn generate_audio_samples(
        &self,
        file: File,
    ) -> Result<(Vec<f32>, u32), Box<dyn std::error::Error>> {
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

        let codec_params = &format.tracks().get(0).unwrap().codec_params;
        let sample_rate = codec_params.sample_rate.unwrap();
        let decoder_options = DecoderOptions::default();

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

            for i in (0..sample_buf.len()).step_by(num_channels) {
                let frame = &sample_buf.samples()[i..i + num_channels];
                let mono_sample = frame.iter().sum::<f32>() / num_channels as f32;
                decoded_audio_samples.push(mono_sample);
            }
        }

        Ok((decoded_audio_samples, sample_rate))
    }

    fn read_return_file(&self, file_path: String) -> File {
        let file = File::open(file_path).unwrap();
        println!("read the file");
        file
    }

    pub fn record_audio(&self, duration_secs: u64) -> (Vec<f32>, SupportedStreamConfig) {
        let host = cpal::default_host();
        let device = host.default_input_device().expect("No input device found");
        let config_cpal = device.default_input_config().unwrap();

        let recorded_samples = Arc::new(Mutex::new(Vec::new()));
        let samples_clone = recorded_samples.clone();

        let err_fn = |err| eprintln!("Stream error: {}", err);

        let stream = match config_cpal.sample_format() {
            cpal::SampleFormat::F32 => device
                .build_input_stream(
                    &config_cpal.clone().into(),
                    move |data: &[f32], _: &_| {
                        samples_clone.lock().unwrap().extend_from_slice(data);
                    },
                    err_fn,
                    None,
                )
                .unwrap(),
            cpal::SampleFormat::I16 => device
                .build_input_stream(
                    &config_cpal.clone().into(),
                    move |data: &[i16], _: &_| {
                        let mut samples = samples_clone.lock().unwrap();
                        for &sample in data.iter() {
                            samples.push(sample as f32 / i16::MAX as f32);
                        }
                    },
                    err_fn,
                    None,
                )
                .unwrap(),
            _ => panic!("Unsupported sample format"),
        };

        stream.play().unwrap();
        thread::sleep(Duration::from_secs(duration_secs));
        drop(stream);

        (recorded_samples.lock().unwrap().clone(), config_cpal)
    }
    pub fn play_recording(&self, recorded_samples: Vec<f32>, config: &StreamConfig) {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("No output device available.");

        println!("Output config: {:?}", config);

        let duration_secs =
            recorded_samples.len() as f32 / (config.sample_rate.0 as f32 * config.channels as f32);

        let mut samples_iter = recorded_samples.into_iter();

        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    // Fill the output buffer with our recorded samples.
                    for sample in data.iter_mut() {
                        *sample = samples_iter.next().unwrap_or(0.0);
                    }
                },
                |err| eprintln!("An error occurred on the output stream: {}", err),
                None,
            )
            .expect("Failed to build output stream.");

        stream.play().unwrap();

        // Keep the main thread alive for the duration of the playback.
        println!("🎵 Playing back for {:.2} seconds...", duration_secs);
        thread::sleep(Duration::from_secs_f32(duration_secs + 1.0));
        println!("Playback finished.");
    }
    pub fn resample_linear(&self, samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
        if from_rate == to_rate {
            return samples.to_vec();
        }
        let ratio = from_rate as f64 / to_rate as f64;
        let new_len = (samples.len() as f64 / ratio) as usize;
        let mut resampled = Vec::with_capacity(new_len);

        for i in 0..new_len {
            let in_idx_float = i as f64 * ratio;
            let in_idx_int = in_idx_float.floor() as usize;
            let frac = in_idx_float.fract() as f32;

            if in_idx_int + 1 < samples.len() {
                let p1 = samples[in_idx_int];
                let p2 = samples[in_idx_int + 1];
                let interpolated = p1 + frac * (p2 - p1);
                resampled.push(interpolated);
            } else if in_idx_int < samples.len() {
                resampled.push(samples[in_idx_int]);
            } else {
                break;
            }
        }
        resampled
    }

    /// This is useful for reducing high-frequency noise, like microphone hiss.
    pub fn apply_low_pass_filter(
        &self,
        samples: &[f32],
        sample_rate: u32,
        cutoff_freq: f32,
    ) -> Vec<f32> {
        if samples.is_empty() {
            return Vec::new();
        }

        // RC time constant for the filter
        let rc = 1.0 / (2.0 * PI * cutoff_freq);
        // Alpha smoothing factor
        let dt = 1.0 / sample_rate as f32;
        let alpha = dt / (rc + dt);

        let mut filtered_samples = vec![0.0; samples.len()];
        filtered_samples[0] = samples[0]; // Start with the first sample

        for i in 1..samples.len() {
            // y[i] = y[i-1] + alpha * (x[i] - y[i-1])
            filtered_samples[i] =
                filtered_samples[i - 1] + alpha * (samples[i] - filtered_samples[i - 1]);
        }

        filtered_samples
    }
}
