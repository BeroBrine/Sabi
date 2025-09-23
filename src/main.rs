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

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(group(
    ArgGroup::new("mode")
        .required(true)
        .args(&["ingest", "recognise", "match" , "random_test"]),
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

    /// Run a test with random snippets from the songs directory
    #[arg(long)]
    random_test: bool,
}

fn main() {
    let args = Args::parse();

    if args.ingest {
        if let Some(file) = args.file {
            ingest_file(file);
        } else {
            eprintln!("Error: --ingest requires --file <path>");
            std::process::exit(1);
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

    // Filter and resample to target
    let filtered_samples =
        audio_processor.apply_low_pass_filter(&audio_samples, sample_rate, 5000.0);
    let rec_resampled = audio_processor.resample_linear(
        &filtered_samples,
        sample_rate,
        AudioProcessor::TARGET_SAMPLE_RATE,
    );

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

/// Ingest an audio file using in-memory processing
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

    let (audio_samples, sample_rate) = audio_processor.get_decoded_audio(file_name);

    let filtered_samples =
        audio_processor.apply_low_pass_filter(&audio_samples, sample_rate, 5000.0);

    let downsampled_samples = audio_processor.resample_linear(
        &filtered_samples,
        sample_rate,
        AudioProcessor::TARGET_SAMPLE_RATE,
    );

    println!(
        "Processed to {} samples at {} Hz",
        downsampled_samples.len(),
        AudioProcessor::TARGET_SAMPLE_RATE
    );

    let fft_distribution = fft
        .generate_freq_time_distribution(downsampled_samples, AudioProcessor::TARGET_SAMPLE_RATE);

    let fingerprints = generate_audio_fingerprint(&fft_distribution);
    println!("Generated {} fingerprints", fingerprints.len());

    let song_id = db.write_song(&song_name);
    db.write_fingerprints(song_id, fingerprints);

    println!("✅ Successfully ingested and fingerprinted '{}'", song_name);
}

/// Record audio via microphone and attempt recognition using in-memory processing
fn ingest_audio() {
    let audio_processor = AudioProcessor::new();
    let fft = CooleyTukeyFFT::default();

    let recording_time_duration = 5;
    println!("🎤 Recording for {} seconds...", recording_time_duration);
    let (recorded_samples, config) = audio_processor.record_audio(recording_time_duration);

    println!("-- Applying Low Pass Filter");
    let filtered_samples =
        audio_processor.apply_low_pass_filter(&recorded_samples, config.sample_rate().0, 5000.0);

    println!("-- Downsampling Audio");
    let downsampled_samples = audio_processor.resample_linear(
        &filtered_samples,
        config.sample_rate().0,
        AudioProcessor::TARGET_SAMPLE_RATE,
    );

    println!(
        "Processed to {} samples at {} Hz",
        downsampled_samples.len(),
        AudioProcessor::TARGET_SAMPLE_RATE
    );

    println!("-- Generating FFT Distribution");
    let fft_distribution = fft
        .generate_freq_time_distribution(downsampled_samples, AudioProcessor::TARGET_SAMPLE_RATE);

    let fingerprints = generate_audio_fingerprint(&fft_distribution);
    println!("Generated {} fingerprints", fingerprints.len());

    let hash_vec: Vec<i64> = fingerprints.iter().map(|f| f.hash as i64).collect();
    let mut db = DB::new();
    println!("-- Fetching Hash Matches From DB");
    let db_matches_by_hash = db.fetch_matches_grouped_by_hash(&hash_vec);
    println!("-- Voting For The Best Matching Result");
    let results = vote_best_matches(&fingerprints, &db_matches_by_hash, 5);

    if results.is_empty() {
        println!("❌ No matches found");
    } else {
        let song_ids: Vec<i32> = results.iter().map(|r| r.song_id as i32).collect();
        let titles = db.fetch_song_titles(&song_ids);

        println!("✅ Top matches:");
        for r in results {
            let title = titles
                .get(&(r.song_id as i32))
                .cloned()
                .unwrap_or_else(|| "<unknown>".to_string());

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
