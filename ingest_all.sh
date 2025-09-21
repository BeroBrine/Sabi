#!/bin/bash

# This script ingests all songs from a specified directory.

# --- Configuration ---
# The directory where your song files are located.
SONGS_DIR="songs"

# --- Main Script ---

# 1. Check if the songs directory exists.
if [ ! -d "$SONGS_DIR" ]; then
  echo "Error: Directory '$SONGS_DIR' not found."
  echo "Please create the directory and place your song files inside."
  exit 1
fi

# 2. Loop through every file in the songs directory.
# The `"$SONGS_DIR"/*` glob pattern expands to all files and directories.
diesel migration redo
echo "Starting song ingestion process..."
for song_file in "$SONGS_DIR"/*
do
  # 3. Check if the item is a file (and not a directory).
  if [ -f "$song_file" ]; then
    echo "---------------------------------"
    echo "🎵 Processing: $song_file"
    
    # 4. Execute the cargo command for the current file.
    # The double dash `--` separates cargo's arguments from your program's arguments.
    cargo run --release -- --ingest --file "$song_file"
    
    echo "" # Add a blank line for cleaner output
  fi
done

echo "✅ Ingestion complete for all songs."
