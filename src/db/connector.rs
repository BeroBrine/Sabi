use crate::{
    db::bindings::{Fingerprint, NewSong, Songs},
    fingerprint::{self, FingerprintInfo},
};
use anyhow::Result;
use diesel::{dsl::insert_into, prelude::*, select, upsert::on_constraint};
use dotenvy::dotenv;
use std::{env, time::SystemTime};

pub struct DB {
    pub connector: PgConnection,
}

impl DB {
    pub fn new() -> Self {
        dotenv().ok();

        let db_url = env::var("DATABASE_URL").expect("Db url must be set");

        let conn = PgConnection::establish(&db_url)
            .unwrap_or_else(|_| panic!("Error connecting to {} ", db_url));

        Self { connector: conn }
    }

    pub fn write_song(&mut self, song_name: &String) -> i32 {
        use crate::schema::songs::dsl::*;

        let song = NewSong {
            title: song_name.clone(),
            created_at: Some(SystemTime::now()),
        };

        let inserted_record = insert_into(songs)
            .values(&song)
            .get_result::<Songs>(&mut self.connector)
            .unwrap();

        println!("inserted record {:?} ", inserted_record);
        inserted_record.id
    }

    pub fn write_fingerprints(&mut self, _song_id: i32, fingerprint_info: Vec<FingerprintInfo>) {
        use crate::schema::fingerprint::dsl::*;

        const BATCH_SIZE: usize = 5000;

        // --- Deduplicate (hash, time) per song ---
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        let mut fingerprints: Vec<Fingerprint> = Vec::new();

        for f_info in fingerprint_info {
            let key = (
                f_info.hash,
                (f_info.abs_anchor_tm_offset * 100.0).round() as i64,
            ); // bucket time
            if seen.insert(key) {
                fingerprints.push(Fingerprint {
                    hash: f_info.hash as i64,
                    absolute_time_offset: f_info.abs_anchor_tm_offset as f64,
                    song_id: _song_id,
                    created_at: Some(SystemTime::now()),
                });
            }
        }

        let mut all_inserted_records = Vec::new();

        let result: Result<Vec<Fingerprint>> = self.connector.transaction(|conn| {
            for batch in fingerprints.chunks(BATCH_SIZE) {
                let inserted_record_fingerprint = insert_into(fingerprint)
                    .values(batch)
                    .on_conflict(on_constraint("fingerprint_pkey"))
                    .do_nothing()
                    .get_result::<Fingerprint>(conn)?;

                println!("Pushed {} ", batch.len());
                all_inserted_records.push(inserted_record_fingerprint);
            }
            Ok(all_inserted_records)
        });

        match result {
            Ok(records) => println!("Successfully inserted {} total records", records.len()),
            Err(e) => eprintln!("Transaction failed {:?} ", e),
        }
    }
    pub fn fetch_matches_grouped_by_hash(
        &mut self,
        hashes_in: &Vec<i64>,
    ) -> std::collections::HashMap<u64, Vec<(u32, f32)>> {
        use crate::schema::fingerprint::dsl::*;

        let records: Vec<Fingerprint> = fingerprint
            .select(fingerprint::all_columns())
            .filter(hash.eq_any(hashes_in))
            .get_results(&mut self.connector)
            .unwrap();

        let mut map: std::collections::HashMap<u64, Vec<(u32, f32)>> =
            std::collections::HashMap::new();

        println!("matching fingerprint {:?} ", records);
        for rec in records {
            let h = rec.hash as u64;
            let db_time = rec.absolute_time_offset as f32;
            let sid = rec.song_id as u32;
            map.entry(h).or_insert_with(Vec::new).push((sid, db_time));
        }

        map
    }

    pub fn fetch_song_titles(
        &mut self,
        song_ids: &[i32],
    ) -> std::collections::HashMap<i32, String> {
        use crate::schema::songs::dsl::*;

        if song_ids.is_empty() {
            return std::collections::HashMap::new();
        }

        let rows: Vec<Songs> = songs
            .select(songs::all_columns())
            .filter(id.eq_any(song_ids))
            .get_results(&mut self.connector)
            .unwrap_or_default();

        let mut map = std::collections::HashMap::new();
        for row in rows {
            map.insert(row.id, row.title);
        }
        map
    }
}
