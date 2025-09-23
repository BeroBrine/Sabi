#!/bin/bash

# Song Downloader Script
#
# Prerequisites:
#   - yt-dlp: A powerful video/audio downloader.
#     Installation:
#       - Using pip (Python): python3 -m pip install -U yt-dlp
#       - On macOS (Homebrew): brew install yt-dlp
#       - On Debian/Ubuntu: sudo apt install yt-dlp
#   - ffmpeg: Required by yt-dlp for audio extraction and conversion.
#     Installation:
#       - On macOS (Homebrew): brew install ffmpeg
#       - On Debian/Ubuntu: sudo apt install ffmpeg
#
# Usage:
#   1. Make sure you have yt-dlp and ffmpeg installed.
#   2. Create a file named 'song_list.txt' in the same directory as this script.
#   3. Add one song name per line in 'song_list.txt'.
#   4. Run the script from your terminal: ./download_songs.sh

# --- Configuration ---
# The directory where your song files will be saved.
SONGS_DIR="../songs"

# The name of the file containing the list of songs to download.
SONG_LIST_FILE="song_list.txt"



# 1. Check if yt-dlp is installed.
if ! command -v yt-dlp &> /dev/null
then
    echo "❌ Error: yt-dlp is not installed."
    echo "Please install it to continue. See installation instructions in the script header."
    exit 1
fi

# 2. Check if ffmpeg is installed.
if ! command -v ffmpeg &> /dev/null
then
    echo "❌ Error: ffmpeg is not installed."
    echo "yt-dlp requires ffmpeg to extract audio. Please install it."
    exit 1
fi

# 3. Check if the song list file exists.
if [ ! -f "$SONG_LIST_FILE" ]; then
    echo "❌ Error: Song list file '$SONG_LIST_FILE' not found."
    echo "Please create this file and add the songs you want to download, one per line."
    exit 1
fi



if [ ! -d "$SONGS_DIR" ]; then
  echo "📁 Directory '$SONGS_DIR' not found. Creating it..."
  mkdir -p "$SONGS_DIR"
fi
cd ..

echo "🎵 Starting song download process..."
echo "================================="
# The `|| [[ -n "$song_query" ]]` handles files that don't end with a newline.
while IFS= read -r song_query || [[ -n "$song_query" ]]; do
  # Skip empty lines
  if [ -z "$song_query" ]; then
    continue
  fi

  echo "🔎 Processing: '$song_query'"

  #  Use yt-dlp to download the audio.
  #    - It searches YouTube for the query and picks the first result.
  #    - It extracts the audio into mp3 format.
  #    - The key flag is '--no-overwrites' (-n), which automatically skips
  #      the download if a file with the same name already exists.
  yt-dlp \
    --extract-audio \
    --audio-format mp3 \
    --audio-quality 0 \
    --no-overwrites \
    --output "$SONGS_DIR/%(title)s.%(ext)s" \
    "ytsearch1:$song_query"

  echo "---------------------------------"

done < "$SONG_LIST_FILE"

echo "✅ Download process complete."
echo "Your songs are in the '$SONGS_DIR' directory."
