use crate::audio_processor::AudioProcessor;
use crate::db::connector::DB;
use crate::fft::fft::CooleyTukeyFFT;
use crate::fingerprint::{generate_audio_fingerprint, vote_best_matches};
use cpal::StreamConfig;
use rand::Rng;
use std::fs;
use std::process::Command; // <-- Add this

/// Runs a comprehensive test by taking random snippets from each song
/// and processing them through the full recognition pipeline.
pub fn run_random_snippet_test(songs_dir: &str) {
    let audio_processor = AudioProcessor::new();
    let fft = CooleyTukeyFFT::default();
    let mut db = DB::new();

    let mut total_tests = 0;
    let mut correct_matches = 0;
    const SNIPPETS_PER_SONG: u32 = 3;
    const SNIPPET_DURATION_SECS: u64 = 10;
    const SNIPPET_TEMP_PATH: &str = "temp_test_snippet.wav";

    println!("🎵 Starting random snippet test...");
    println!("   Snippets per song: {}", SNIPPETS_PER_SONG);
    println!("   Snippet duration: {}s", SNIPPET_DURATION_SECS);

    let song_entries = match fs::read_dir(songs_dir) {
        Ok(entries) => entries.collect::<Result<Vec<_>, _>>().unwrap_or_default(),
        Err(e) => {
            eprintln!("Error reading songs directory '{}': {}", songs_dir, e);
            return;
        }
    };

    for entry in song_entries {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let file_path_str = path.to_string_lossy().to_string();
        let true_song_name = path.file_name().unwrap().to_string_lossy().to_string();

        println!("\n--- Testing: {} ---", true_song_name);

        // --- 1. Get song duration with ffprobe instead of loading the whole file ---
        let ffprobe_output = Command::new("ffprobe")
            .arg("-v")
            .arg("error")
            .arg("-show_entries")
            .arg("format=duration")
            .arg("-of")
            .arg("default=noprint_wrappers=1:nokey=1")
            .arg(&file_path_str)
            .output();

        let duration_str = match ffprobe_output {
            Ok(output) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            }
            _ => {
                println!("   -> Skipping, failed to get duration with ffprobe.");
                continue;
            }
        };

        let duration_secs = match duration_str.parse::<f64>() {
            Ok(d) => d,
            Err(_) => {
                println!(
                    "   -> Skipping, failed to parse duration '{}'.",
                    duration_str
                );
                continue;
            }
        };

        // Ensure song is long enough for a snippet
        if duration_secs < (SNIPPET_DURATION_SECS + 5) as f64 {
            println!("   -> Skipping, song is too short.");
            continue;
        }

        for i in 0..SNIPPETS_PER_SONG {
            total_tests += 1;

            // --- 2. Calculate a random start time and use FFmpeg to extract the snippet ---
            let max_start_time = duration_secs as u64 - SNIPPET_DURATION_SECS;
            let start_time = rand::thread_rng().gen_range(0..=max_start_time);

            print!("   Snippet #{} (starts at {}s): ", i + 1, start_time);

            let ffmpeg_status = Command::new("ffmpeg")
                .arg("-y")
                .arg("-ss") // Seek to start time
                .arg(start_time.to_string())
                .arg("-t") // Set duration
                .arg(SNIPPET_DURATION_SECS.to_string())
                .arg("-i")
                .arg(&file_path_str)
                .arg("-c:a") // Set audio codec
                .arg("pcm_s16le")
                .arg("-ar") // Set audio sample rate
                .arg("11000")
                .arg("-ac") // Set audio channels
                .arg("1") // Mono
                .arg(SNIPPET_TEMP_PATH)
                .status();

            if ffmpeg_status.is_err() || !ffmpeg_status.unwrap().success() {
                println!("❌ FFmpeg snippet extraction failed.");
                continue;
            }

            // --- 3. Load the normalized snippet and run the recognition pipeline ---
            let (snippet_samples, sample_rate) =
                audio_processor.get_decoded_audio(SNIPPET_TEMP_PATH.to_string());

            let config = StreamConfig {
                channels: 1,
                sample_rate: cpal::SampleRate(11000),
                buffer_size: cpal::BufferSize::Default,
            };
            // audio_processor.play_recording(snippet_samples.clone(), &config);

            // NOTE: No need for filtering or resampling here, FFmpeg already did it!
            let fft_distribution =
                fft.generate_freq_time_distribution(snippet_samples, sample_rate);
            let fingerprints = generate_audio_fingerprint(&fft_distribution);

            if fingerprints.is_empty() {
                println!("❌ No fingerprints generated, match failed.");
                continue;
            }

            let hash_vec: Vec<i64> = fingerprints.iter().map(|f| f.hash as i64).collect();
            let db_matches_by_hash = db.fetch_matches_grouped_by_hash(&hash_vec);
            let results = vote_best_matches(&fingerprints, &db_matches_by_hash, 1);

            // --- 4. Check the result ---
            if let Some(best_match) = results.first() {
                let titles = db.fetch_song_titles(&[best_match.song_id as i32]);
                let predicted_name = titles.get(&(best_match.song_id as i32)).unwrap();

                if predicted_name == &true_song_name {
                    println!("✅ Correct! (score: {})", best_match.score);
                    correct_matches += 1;
                } else {
                    println!(
                        "❌ Incorrect. Matched '{}' (score: {})",
                        predicted_name, best_match.score
                    );
                }
            } else {
                println!("❌ No match found in DB.");
            }
        }
    }

    // --- 5. Final cleanup ---
    let _ = fs::remove_file(SNIPPET_TEMP_PATH);

    println!("\n--- 📊 Test Finished ---");
    if total_tests > 0 {
        let accuracy = (correct_matches as f32 / total_tests as f32) * 100.0;
        println!(
            "   Correct Matches: {} / {}\n   Accuracy: {:.2}%",
            correct_matches, total_tests, accuracy
        );
    } else {
        println!("No tests were run. Check the songs directory path.");
    }
}
