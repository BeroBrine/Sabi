-- Your SQL goes here

CREATE TABLE songs (
  id SERIAL PRIMARY KEY,
  title VARCHAR(255) NOT NULL,
  created_at TIMESTAMP DEFAULT NOW()
);

CREATE TABLE fingerprint (
  hash BIGINT NOT NULL, 
  absolute_time_offset FLOAT NOT NULL,
  song_id INT NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
  created_at TIMESTAMP DEFAULT NOW(),
  PRIMARY KEY (song_id , absolute_time_offset , hash)
);

CREATE INDEX idx_fingerprint_hash ON fingerprint(hash);
