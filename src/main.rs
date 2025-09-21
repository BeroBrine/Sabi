mod audio_processor;
mod db;
mod fft;
mod fingerprint;
mod schema;
mod tester;

use crate::db::connector::DB;
use crate::fingerprint::{generate_audio_fingerprint, vote_best_matches};
use crate::{audio_processor::AudioProcessor, fft::fft::CooleyTukeyFFT};
use clap::{ArgGroup, Parser};
use cpal::StreamConfig;
use cpal::traits::{DeviceTrait, HostTrait};
use std::fs;
use std::process::Command;

/// Audio Fingerprinting CLI
///
/// You can either ingest a file into the database, or
/// record audio for recognition. These modes are mutually exclusive.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(group(
    ArgGroup::new("mode")
        .required(true)
        .args(&["ingest", "recognise", "match" , "batch_test" , "random_test"]),
))]
struct Args {
    /// Ingest a file into the database
    #[arg(long)]
    ingest: bool,

    /// Recognise audio from microphone input
    #[arg(long)]
    recognise: bool,

    /// Match a snippet file against DB
    #[arg(long, id = "match")]
    match_: bool,

    /// Path to the audio file (required for --ingest and --match)
    #[arg(short, long)]
    file: Option<String>,

    /// Batch test: generate snippets from each ingested song and test recognition
    #[arg(long)]
    batch_test: bool,

    #[arg(long)]
    random_test: bool,
}

fn main() {
    let args = Args::parse();

    if args.ingest {
        if args.ingest {
            if let Some(file) = args.file {
                ingest_file(file);
            } else {
                eprintln!("Error: --ingest requires --file <path>");
                std::process::exit(1);
            }
        }
    } else if args.recognise {
        ingest_audio();
    } else if args.match_ {
        if let Some(file) = args.file {
            match_file(file);
        } else {
            eprintln!("Error: --match requires --file <path>");
            std::process::exit(1);
        }
    } else if args.random_test {
        if let Some(dir) = args.file {
            tester::run_random_snippet_test(&dir);
        } else {
            eprintln!("Error: --random-test requires --file <songs_dir>");
            std::process::exit(1);
        }
    }
}

/// Decode a snippet file and try to match against DB
fn match_file(file_name: String) {
    let audio_processor = AudioProcessor::new();
    let fft = CooleyTukeyFFT::default();

    // Decode snippet
    let (audio_samples, sample_rate) = audio_processor.get_decoded_audio(file_name.clone());
    println!(
        "Loaded snippet {} samples @ {} Hz",
        audio_samples.len(),
        sample_rate
    );

    // Resample to target
    let target_sr = AudioProcessor::TARGET_SAMPLE_RATE;
    let rec_resampled = if sample_rate != target_sr {
        audio_processor.resample_linear(&audio_samples, sample_rate, target_sr)
    } else {
        audio_samples
    };

    // FFT distribution
    let fft_distribution =
        fft.generate_freq_time_distribution(rec_resampled, AudioProcessor::TARGET_SAMPLE_RATE);

    // Fingerprint
    let fingerprints = generate_audio_fingerprint(&fft_distribution);
    println!("Generated {} fingerprints", fingerprints.len());

    // Query DB
    let hash_vec: Vec<i64> = fingerprints.iter().map(|f| f.hash as i64).collect();
    let mut db = DB::new();
    let db_matches_by_hash = db.fetch_matches_grouped_by_hash(&hash_vec);

    // Vote
    let results = vote_best_matches(&fingerprints, &db_matches_by_hash, 5);

    if results.is_empty() {
        println!("❌ No matches found");
    } else {
        // Fetch titles
        let song_ids: Vec<i32> = results.iter().map(|r| r.song_id as i32).collect();
        let titles = db.fetch_song_titles(&song_ids);

        println!("✅ Top matches:");
        for r in results {
            let title = titles
                .get(&(r.song_id as i32))
                .cloned()
                .unwrap_or_else(|| "<unknown>".to_string());
            println!(
                "  id={} title=\"{}\" score={} offset={:.2}s",
                r.song_id, title, r.score, r.time_offset
            );
        }
    }
}

/// Ingest an audio file: decode, run FFT, fingerprint, and store in DB.
// src/main.rs

// ...

/// Ingest an audio file: DECODE -> NORMALIZE WITH FFMPEG -> fingerprint, and store in DB.
fn ingest_file(file_name: String) {
    let song_name = file_name
        .rsplit('/')
        .next()
        .unwrap_or("Unknown Song")
        .to_string();

    println!("Ingesting song: {}", song_name);

    let mut db = DB::new();
    let audio_processor = AudioProcessor::new();
    let fft = CooleyTukeyFFT::default();

    // --- NEW: Use FFmpeg to normalize the input file ---
    const NORMALIZED_INGEST_PATH: &str = "temp_normalized_ingest.wav";
    println!("⚙️ Normalizing '{}' with FFmpeg...", song_name);
    let ffmpeg_status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(&file_name) // Use the input file name here
        .arg("-c:a")
        .arg("pcm_s16le")
        .arg("-ar")
        .arg("11000")
        .arg("-ac")
        .arg("1")
        .arg(NORMALIZED_INGEST_PATH)
        .status();

    if ffmpeg_status.is_err() || !ffmpeg_status.unwrap().success() {
        eprintln!(
            "❌ FFmpeg normalization failed for {}. Skipping.",
            file_name
        );
        return;
    }

    // --- Load the clean, normalized audio ---
    let (normalized_samples, normalized_sample_rate) =
        audio_processor.get_decoded_audio(NORMALIZED_INGEST_PATH.to_string());

    println!(
        "Normalized to {} samples at {} Hz",
        normalized_samples.len(),
        normalized_sample_rate
    );

    let config = StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(11000),
        buffer_size: cpal::BufferSize::Default,
    };
    // audio_processor.play_recording(normalized_samples.clone(), &config);

    // --- The rest of the pipeline uses the normalized data ---
    let fft_distribution =
        fft.generate_freq_time_distribution(normalized_samples, normalized_sample_rate);

    let fingerprints = generate_audio_fingerprint(&fft_distribution);

    let song_id = db.write_song(&song_name);
    db.write_fingerprints(song_id, fingerprints);

    println!("✅ Successfully ingested and fingerprinted '{}'", song_name);

    // --- IMPORTANT: Clean up the temporary file ---
    let _ = fs::remove_file(NORMALIZED_INGEST_PATH);
}
/// Record audio via microphone and attempt recognition.
// src/main.rs

// Add this use statement at the top

// ...

fn ingest_audio() {
    let audio_processor = AudioProcessor::new();

    // --- 1. Record audio from the mic as before ---
    let recording_time_duration = 12;
    println!("🎤 Recording for {} seconds...", recording_time_duration);
    let (recorded_samples, config) = audio_processor.record_audio(recording_time_duration);

    // let stream_config = StreamConfig {
    //     channels: 1,
    //     sample_rate: cpal::SampleRate(44_000),
    //     buffer_size: cpal::BufferSize::Default,
    // };
    // audio_processor.play_recording(recorded_samples.clone(), &stream_config);

    // --- 2. Save the raw recording to a temporary WAV file ---
    const RAW_WAV_PATH: &str = "temp_raw_recording.wav";
    if let Err(e) = audio_processor.save_as_wav(&recorded_samples, &config, RAW_WAV_PATH) {
        eprintln!("❌ Failed to save raw audio: {}", e);
        return;
    }

    // --- 3. Use FFmpeg to normalize the audio ---
    const NORMALIZED_WAV_PATH: &str = "temp_normalized_recording.wav";
    println!("⚙️ Normalizing audio with FFmpeg...");
    let ffmpeg_status = Command::new("ffmpeg")
        .arg("-y") // Overwrite output file if it exists
        .arg("-i")
        .arg(RAW_WAV_PATH)
        .arg("-c:a") // Specify audio codec
        .arg("pcm_s16le") // 16-bit PCM audio
        .arg("-ar") // Set audio sample rate
        .arg("11000")
        .arg("-ac") // Set number of audio channels
        .arg("1") // Mono
        .arg(NORMALIZED_WAV_PATH)
        .status();

    if ffmpeg_status.is_err() || !ffmpeg_status.unwrap().success() {
        eprintln!(
            "❌ FFmpeg normalization failed. Make sure FFmpeg is installed and in your PATH."
        );
        // Cleanup temp file before exiting
        let _ = fs::remove_file(RAW_WAV_PATH);
        return;
    }

    // --- 4. Load the clean, normalized audio file for processing ---
    // We can now reuse the file decoding logic!
    println!("🎧 Processing normalized audio...");
    let (normalized_samples, normalized_sample_rate) =
        audio_processor.get_decoded_audio(NORMALIZED_WAV_PATH.to_string());

    let config = StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(11000),
        buffer_size: cpal::BufferSize::Default,
    };
    audio_processor.play_recording(normalized_samples.clone(), &config);

    // --- 5. The rest of the pipeline remains the same, but on the new audio data ---
    // NOTE: We no longer need to filter or resample here, as FFmpeg already did it.
    let fft = CooleyTukeyFFT::default();
    let fft_distribution =
        fft.generate_freq_time_distribution(normalized_samples, normalized_sample_rate);

    let fingerprints = generate_audio_fingerprint(&fft_distribution);
    println!("Generated {} fingerprints", fingerprints.len());

    // --- 6. Query the DB and vote ---
    let hash_vec: Vec<i64> = fingerprints.iter().map(|f| f.hash as i64).collect();
    let mut db = DB::new();
    let db_matches_by_hash = db.fetch_matches_grouped_by_hash(&hash_vec);
    let results = vote_best_matches(&fingerprints, &db_matches_by_hash, 5);

    // ... (rest of the result printing logic)
    if results.is_empty() {
        println!("❌ No matches found");
    } else {
        // Fetch titles for the result song_ids
        let song_ids: Vec<i32> = results.iter().map(|r| r.song_id as i32).collect();
        let titles = db.fetch_song_titles(&song_ids);

        println!("Top matches:");
        for r in results {
            let title = titles
                .get(&(r.song_id as i32))
                .cloned()
                .unwrap_or_else(|| "<unknown>".to_string());

            // Format time offset as MM:SS (handle negative values)
            let abs_offset = r.time_offset.abs();
            let minutes = (abs_offset / 60.0) as u32;
            let seconds = (abs_offset % 60.0) as u32;
            let sign = if r.time_offset < 0.0 { "-" } else { "" };
            let time_str = format!("{}{:02}:{:02}", sign, minutes, seconds);

            println!(
                "song_id={} title=\"{}\" score={} time_offset={}s ({})",
                r.song_id, title, r.score, r.time_offset, time_str
            );
        }
    }

    // --- 7. IMPORTANT: Clean up the temporary files ---
    let _ = fs::remove_file(RAW_WAV_PATH);
    let _ = fs::remove_file(NORMALIZED_WAV_PATH);
}
