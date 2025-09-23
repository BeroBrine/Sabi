use crate::{
    db::bindings::{Fingerprint, FingerprintMatch, NewSong, Songs},
    fingerprint::FingerprintInfo,
};
use diesel::{RunQueryDsl, dsl::insert_into, prelude::*, upsert::on_constraint};
use dotenvy::dotenv;
use std::{collections::HashMap, env, time::SystemTime};

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
        use std::collections::HashSet;

        const BATCH_SIZE: usize = 15_000;

        // --- Deduplicate (hash, time) per song ---
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

        if fingerprints.is_empty() {
            println!("No new fingerprints to write for song_id: {}", _song_id);
            return;
        }

        // --- Transaction to insert fingerprints in batches ---
        let result: Result<usize, diesel::result::Error> = self.connector.transaction(|conn| {
            let mut total_inserted = 0;
            for batch in fingerprints.chunks(BATCH_SIZE) {
                let inserted_count = insert_into(fingerprint)
                    .values(batch)
                    .on_conflict(on_constraint("fingerprint_pkey"))
                    .do_nothing()
                    .execute(conn)?;

                total_inserted += inserted_count;
                println!("Batch executed. Affected rows: {}", inserted_count);
            }
            Ok(total_inserted)
        });

        match result {
            Ok(count) => println!(
                "✅ Successfully committed {} new fingerprints to the database.",
                count
            ),
            Err(e) => eprintln!("❌ Transaction failed: {:?}", e),
        }
    }
    pub fn fetch_matches_grouped_by_hash(
        &mut self,
        hashes_in: &Vec<i64>,
    ) -> std::collections::HashMap<u64, Vec<(u32, f32)>> {
        if hashes_in.is_empty() {
            return std::collections::HashMap::new();
        }

        let records: Vec<FingerprintMatch> = self.connector.transaction(|conn| {
            diesel::sql_query(
                "CREATE TEMPORARY TABLE Temp_hashes (hash BIGINT NOT NULl PRIMARY KEY) ON COMMIT DROP;"
            ).execute(conn).unwrap();
            diesel::table! {
                temp_hashes (hash) {
                    hash -> BigInt,
                }
            }

            #[derive(Insertable)]
            #[diesel(table_name=temp_hashes)]
            struct NewHash {
                hash: i64,
            }

            const BATCH_SIZE: usize = 5000;

            for batch in hashes_in.chunks(BATCH_SIZE) {
                let new_hashes: Vec<NewHash> = batch.iter().map(|&h| NewHash {hash: h}).collect();
                diesel::insert_into(temp_hashes::table).values(&new_hashes).on_conflict_do_nothing().execute(conn).unwrap();
            }

            let query = "
                SELECT
                    f.hash , f.song_id , f.absolute_time_offset
                FROM
                    fingerprint AS f
                INNER JOIN
                    temp_hashes AS t ON f.hash = t.hash;
                ";

            diesel::sql_query(query).load::<FingerprintMatch>(conn)



        }).expect("Transaction failed");

        let mut map: HashMap<u64, Vec<(u32, f32)>> = HashMap::new();

        for rec in records {
            let h = rec.hash as u64;
            let sid = rec.song_id as u32;
            let db_time = rec.absolute_time_offset as f32;
            map.entry(h).or_default().push((sid, db_time));
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
