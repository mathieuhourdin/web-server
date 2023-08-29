// @generated automatically by Diesel CLI.

diesel::table! {
    articles (id) {
        id -> Uuid,
        title -> Text,
        description -> Text,
        content -> Text,
        potential_improvements -> Text,
        author_id -> Nullable<Uuid>,
        progress -> Int4,
        maturing_state -> Text,
        parent_id -> Nullable<Uuid>,
        gdoc_url -> Nullable<Text>,
        image_url -> Nullable<Text>,
        url_slug -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    sessions (id) {
        id -> Uuid,
        user_id -> Nullable<Uuid>,
        token -> Nullable<Text>,
        authenticated -> Bool,
        expires_at -> Timestamp,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    users (id) {
        id -> Uuid,
        email -> Text,
        first_name -> Text,
        last_name -> Text,
        handle -> Text,
        password -> Text,
        created_at -> Timestamp,
        updated_at -> Nullable<Timestamp>,
    }
}

diesel::joinable!(articles -> users (author_id));
diesel::joinable!(sessions -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    articles,
    sessions,
    users,
);
