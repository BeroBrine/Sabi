use std::time::SystemTime;

use diesel::prelude::*;

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = crate::schema::fingerprint)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Fingerprint {
    pub hash: i64,
    pub absolute_time_offset: f64,
    pub song_id: i32,
    pub created_at: Option<SystemTime>,
}

#[derive(Queryable, Selectable, Debug)]
#[diesel(table_name = crate::schema::songs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Songs {
    pub id: i32,
    pub title: String,
    pub created_at: Option<SystemTime>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::songs)]
pub struct NewSong {
    pub title: String,
    pub created_at: Option<SystemTime>,
}
