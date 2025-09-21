use crate::audio_processor::AudioProcessor;
use crate::db::connector::DB;
use crate::fft::fft::CooleyTukeyFFT;
use crate::fingerprint::{generate_audio_fingerprint, vote_best_matches};
use rand::Rng; // For selecting random start times
use std::fs;

/// Runs a comprehensive test by taking random snippets from each song
/// and processing them through the full recognition pipeline.
pub fn run_random_snippet_test(songs_dir: &str) {
    let audio_processor = AudioProcessor::new();
    let fft = CooleyTukeyFFT::default();
    let mut db = DB::new();

    let mut total_tests = 0;
    let mut correct_matches = 0;
    const SNIPPETS_PER_SONG: u32 = 3;
    const SNIPPET_DURATION_SECS: usize = 10;

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

        // 1. Decode the full song once
        let (full_samples, sample_rate) = audio_processor.get_decoded_audio(file_path_str);

        // Ensure song is long enough for a snippet
        let min_len = sample_rate as usize * (SNIPPET_DURATION_SECS + 5);
        if full_samples.len() < min_len {
            println!("   -> Skipping, song is too short.");
            continue;
        }

        for i in 0..SNIPPETS_PER_SONG {
            total_tests += 1;

            // 2. Extract a random snippet
            let snippet_len = SNIPPET_DURATION_SECS * sample_rate as usize;
            let max_start_index = full_samples.len() - snippet_len;
            let start_index = rand::rng().random_range(0..=max_start_index);
            let end_index = start_index + snippet_len;
            let snippet = &full_samples[start_index..end_index];

            let start_time_secs = start_index as f32 / sample_rate as f32;
            print!(
                "   Snippet #{} (starts at {:.2}s): ",
                i + 1,
                start_time_secs
            );

            // 3. Run through the FULL recognition pipeline (resample -> filter -> FFT -> fingerprint -> vote)
            let target_sr = AudioProcessor::TARGET_SAMPLE_RATE;
            let resampled = audio_processor.resample_linear(snippet, sample_rate, target_sr);
            let filtered = audio_processor.apply_low_pass_filter(&resampled, target_sr, 5500.0);
            let fft_distribution = fft.generate_freq_time_distribution(filtered, target_sr);
            let fingerprints = generate_audio_fingerprint(&fft_distribution);

            if fingerprints.is_empty() {
                println!("❌ No fingerprints generated, match failed.");
                continue;
            }

            let hash_vec: Vec<i64> = fingerprints.iter().map(|f| f.hash as i64).collect();
            let db_matches_by_hash = db.fetch_matches_grouped_by_hash(&hash_vec);
            let results = vote_best_matches(&fingerprints, &db_matches_by_hash, 1);

            // 4. Check the result
            if let Some(best_match) = results.first() {
                let titles = db.fetch_song_titles(&[best_match.song_id as i32]);
                let predicted_name = titles.get(&(best_match.song_id as i32)).unwrap();

                if predicted_name == &true_song_name {
                    println!("✅ Correct! (score: {})", best_match.score);
                    println!("✅ The db fetch as {:?} ", results.first().unwrap());
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
