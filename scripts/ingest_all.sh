#!/bin/bash

# This script ingests all songs from a specified directory.

SONGS_DIR="../songs"


#  Check if the songs directory exists.
if [ ! -d "$SONGS_DIR" ]; then
  echo "Error: Directory '$SONGS_DIR' not found."
  echo "Please create the directory and place your song files inside."
  exit 1
fi


diesel migration redo
echo "Starting song ingestion process..."
for song_file in "$SONGS_DIR"/*
do
  if [ -f "$song_file" ]; then
    echo "---------------------------------"
    echo "🎵 Processing: $song_file"
    
    cargo run --release -- --ingest --file "$song_file"
    
    echo "" 
  fi
done

echo "✅ Ingestion complete for all songs."
