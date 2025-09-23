#!/bin/bash

# ==============================================================================
# Sabi - Recognition Tester Script
#
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


SONGS_DIR="../songs"

SNIPPET_DURATION=15

START_MARGIN=10

END_MARGIN=15



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




echo "🚀 Starting Recognition Test..."
echo "---------------------------------"

RANDOM_SONG=$(find "$SONGS_DIR" -type f | shuf -n 1)

if [ -z "$RANDOM_SONG" ]; then
    echo "❌ Error: Could not select a random song. Check the '$SONGS_DIR' directory."
    exit 1
fi

echo "🎵 Selected Song: $RANDOM_SONG"

DURATION=$(ffprobe -i "$RANDOM_SONG" -show_entries format=duration -v quiet -of csv="p=0")
DURATION_INT=${DURATION%.*} # Convert float to integer (e.g., 210.456 -> 210)

MIN_START=$START_MARGIN
MAX_START=$((DURATION_INT - SNIPPET_DURATION - END_MARGIN))

# Check if the song is long enough for a valid snippet.
if [ "$MAX_START" -le "$MIN_START" ]; then
    echo "⚠️ Warning: The selected song is too short to pick a random snippet from the middle."
    echo "Please try with longer audio files. Skipping this run."
    exit 1
fi

RANDOM_START=$((RANDOM % (MAX_START - MIN_START + 1) + MIN_START))

echo "🎧 Playing a $SNIPPET_DURATION-second snippet starting at $RANDOM_START seconds."

ffplay -v quiet -nodisp -autoexit -ss "$RANDOM_START" -t "$SNIPPET_DURATION" "$RANDOM_SONG" &
FFPLAY_PID=$! 

sleep 0.5

echo "🎤 Snippet playing. Starting recognition immediately..."
echo "================================================"

RUSTFLAGS=-Awarnings cargo run --release -- --recognise


wait $FFPLAY_PID

echo ""
echo "✅ Recognition test complete."

