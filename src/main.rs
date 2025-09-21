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
use std::fs;

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
fn ingest_file(file_name: String) {
    // Extract the song name from the path
    let song_name = file_name
        .rsplit('/')
        .next()
        .unwrap_or("Unknown Song")
        .to_string();

    println!("Ingesting song: {}", song_name);

    let mut db = DB::new();
    let audio_processor = AudioProcessor::new();
    let fft = CooleyTukeyFFT::default();

    // Decode audio file into raw samples
    let (audio_samples, sample_rate) = audio_processor.get_decoded_audio(file_name);
    println!(
        "Decoded {} samples at {} Hz",
        audio_samples.len(),
        sample_rate
    );

    let downsampled = audio_processor.resample_linear(
        &audio_samples,
        sample_rate,
        AudioProcessor::TARGET_SAMPLE_RATE,
    );

    // Compute FFT time-frequency distribution
    let fft_distribution =
        fft.generate_freq_time_distribution(downsampled, AudioProcessor::TARGET_SAMPLE_RATE);

    // Generate fingerprints
    let fingerprints = generate_audio_fingerprint(&fft_distribution);

    // Write song + fingerprints to DB
    let song_id = db.write_song(&song_name);
    db.write_fingerprints(song_id, fingerprints);

    println!("Successfully ingested and fingerprinted '{}'", song_name);

    // Optional: visualization (uncomment if needed)
    // if let Err(e) = write_heatmap_svg(&fft_distribution, "heatmap.svg", &song_name) {
    //     eprintln!("Failed to write heatmap.svg: {}", e);
    // } else {
    //     println!("Wrote heatmap.svg");
    // }
}

/// Record audio via microphone and attempt recognition.
fn ingest_audio() {
    let audio_processor = AudioProcessor::new();

    let recording_time_duration = 6;
    println!("Recording for {} seconds...", recording_time_duration);
    let (recorded_samples, config) = audio_processor.record_audio(recording_time_duration);

    println!("Playback recorded audio...");
    audio_processor.play_recording(recorded_samples.clone(), &config.clone().into());

    // Compute FFT time-frequency distribution
    let fft = CooleyTukeyFFT::default();
    // Resample recorded audio to target sample rate used in decoder for consistency
    let target_sr = AudioProcessor::TARGET_SAMPLE_RATE;
    let rec_resampled = if config.sample_rate().0 != target_sr {
        let out =
            audio_processor.resample_linear(&recorded_samples, config.sample_rate().0, target_sr);

        out
    } else {
        recorded_samples
    };

    let filtered_samples = audio_processor.apply_low_pass_filter(
        &rec_resampled,
        target_sr,
        5500.0, // Cutoff frequency in Hz. Good for removing hiss without losing musical detail.
    );

    let fft_distribution =
        fft.generate_freq_time_distribution(filtered_samples, AudioProcessor::TARGET_SAMPLE_RATE);

    // Generate fingerprints
    let fingerprints = generate_audio_fingerprint(&fft_distribution);
    println!("Generated {} fingerprints", fingerprints.len());

    // Debug: Show some sample fingerprints
    for (i, fp) in fingerprints.iter().take(5).enumerate() {
        println!(
            "Fingerprint {}: hash={}, time={}",
            i, fp.hash, fp.abs_anchor_tm_offset
        );
    }

    let hash_vec: Vec<i64> = fingerprints.iter().map(|f| f.hash as i64).collect();
    let mut db = DB::new();

    // Fetch DB matches grouped by hash => Vec<(song_id, db_time)>
    let db_matches_by_hash = db.fetch_matches_grouped_by_hash(&hash_vec);

    // Rank candidates using voting (0.1s bucket for better precision, top 5 results)
    let results = vote_best_matches(&fingerprints, &db_matches_by_hash, 5);

    if results.is_empty() {
        println!("No matches found");
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
}
