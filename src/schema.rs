// @generated automatically by Diesel CLI.

diesel::table! {
    fingerprint (song_id, absolute_time_offset) {
        hash -> Int8,
        absolute_time_offset -> Float8,
        song_id -> Int4,
        created_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    songs (id) {
        id -> Int4,
        #[max_length = 255]
        title -> Varchar,
        created_at -> Nullable<Timestamp>,
    }
}

diesel::joinable!(fingerprint -> songs (song_id));

diesel::allow_tables_to_appear_in_same_query!(fingerprint, songs,);
