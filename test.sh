#!/bin/bash

# ==============================================================================
# Sabi - Recognition Tester Script
#
# Description:
#   This script automates the process of testing the audio recognition.
#   It performs the following steps:
#     1. Selects a random song from the specified directory.
#     2. Calculates a random start time, avoiding the very beginning and end.
#     3. Plays a short snippet of the song in the background.
#     4. Immediately runs the 'recognise' command of the Rust project, which
#        listens while the snippet is playing.
#
# Prerequisites:
#   - ffmpeg/ffplay: For audio processing and playback.
#     Installation:
#       - On macOS (Homebrew): brew install ffmpeg
#       - On Debian/Ubuntu: sudo apt install ffmpeg
#
# Usage:
#   1. Make sure you have songs in the 'songs' directory.
#   2. Run the script from your terminal: ./test_recognition.sh
# ==============================================================================

# --- Configuration ---
# The directory where your song files are located.
SONGS_DIR="songs"

# How long the audio snippet should be (in seconds).
# This should match the recording duration in your Rust app.
SNIPPET_DURATION=6

# How many seconds to skip at the beginning of the song to avoid silence/intros.
START_MARGIN=10

# A buffer to ensure the snippet doesn't run past the end of the song.
END_MARGIN=15


# --- Pre-flight Checks ---

# 1. Check if ffplay is installed for playback.
if ! command -v ffplay &> /dev/null; then
    echo "❌ Error: ffplay is not installed. It's part of the ffmpeg suite."
    echo "Please install ffmpeg to continue."
    exit 1
fi

# 2. Check if ffprobe is installed to get song duration.
if ! command -v ffprobe &> /dev/null; then
    echo "❌ Error: ffprobe is not installed. It's part of the ffmpeg suite."
    echo "Please install ffmpeg to continue."
    exit 1
fi

# 3. Check if the songs directory exists.
if [ ! -d "$SONGS_DIR" ]; then
    echo "❌ Error: Directory '$SONGS_DIR' not found."
    echo "Please create the directory and add some songs first."
    exit 1
fi

# 4. Check if there are any files in the songs directory.
if [ -z "$(ls -A "$SONGS_DIR")" ]; then
    echo "❌ Error: The '$SONGS_DIR' directory is empty."
    echo "Please add some music files to test."
    exit 1
fi


# --- Main Script ---

echo "🚀 Starting Recognition Test..."
echo "---------------------------------"

# 1. Select a random song file from the directory.
# `find` gets all files, `shuf -n 1` picks one randomly.
RANDOM_SONG=$(find "$SONGS_DIR" -type f | shuf -n 1)

if [ -z "$RANDOM_SONG" ]; then
    echo "❌ Error: Could not select a random song. Check the '$SONGS_DIR' directory."
    exit 1
fi

echo "🎵 Selected Song: $RANDOM_SONG"

# 2. Get the duration of the song in seconds using ffprobe.
# The output is cleaned up to be an integer.
DURATION=$(ffprobe -i "$RANDOM_SONG" -show_entries format=duration -v quiet -of csv="p=0")
DURATION_INT=${DURATION%.*} # Convert float to integer (e.g., 210.456 -> 210)

# 3. Calculate a valid range for the random starting point.
MIN_START=$START_MARGIN
MAX_START=$((DURATION_INT - SNIPPET_DURATION - END_MARGIN))

# Check if the song is long enough for a valid snippet.
if [ "$MAX_START" -le "$MIN_START" ]; then
    echo "⚠️ Warning: The selected song is too short to pick a random snippet from the middle."
    echo "Please try with longer audio files. Skipping this run."
    exit 1
fi

# 4. Generate a random starting second within the calculated range.
RANDOM_START=$((RANDOM % (MAX_START - MIN_START + 1) + MIN_START))

echo "🎧 Playing a $SNIPPET_DURATION-second snippet starting at $RANDOM_START seconds."
echo "(Your microphone will pick this up for recognition)"

# 5. Play the audio snippet in the background using ffplay.
#    The '&' at the end is the crucial change that runs this command in the background.
ffplay -v quiet -nodisp -autoexit -ss "$RANDOM_START" -t "$SNIPPET_DURATION" "$RANDOM_SONG" &
FFPLAY_PID=$! # Store the process ID of the background job

# Give ffplay a moment to buffer and start playing before we listen.
sleep 0.5

echo "🎤 Snippet playing. Starting recognition immediately..."
echo "================================================"

# 6. Run the recognition command for your Rust project.
#    This will now run WHILE the audio is playing in the background.
cargo run -- --recognise

# 7. Wait for the ffplay process to finish to ensure a clean exit.
wait $FFPLAY_PID

echo ""
echo "✅ Recognition test complete."

