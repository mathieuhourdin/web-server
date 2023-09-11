// @generated automatically by Diesel CLI.

diesel::table! {
    comments (id) {
        id -> Uuid,
        content -> Text,
        thought_output_id -> Uuid,
        comment_type -> Nullable<Text>,
        start_index -> Nullable<Int4>,
        end_index -> Nullable<Int4>,
        parent_id -> Nullable<Uuid>,
        editing -> Bool,
        author_id -> Uuid,
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
    thought_inputs (id) {
        id -> Uuid,
        resource_title -> Text,
        resource_author_name -> Text,
        resource_type -> Nullable<Text>,
        resource_link -> Nullable<Text>,
        resource_image_link -> Nullable<Text>,
        resource_comment -> Text,
        input_progress -> Int4,
        input_date -> Nullable<Timestamp>,
        input_comment -> Text,
        input_is_public -> Bool,
        input_user_id -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    thought_outputs (id) {
        id -> Uuid,
        title -> Text,
        description -> Text,
        content -> Text,
        potential_improvements -> Text,
        author_id -> Nullable<Uuid>,
        progress -> Int4,
        maturing_state -> Text,
        publishing_state -> Text,
        parent_id -> Nullable<Uuid>,
        gdoc_url -> Nullable<Text>,
        image_url -> Nullable<Text>,
        url_slug -> Nullable<Text>,
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

diesel::joinable!(comments -> thought_outputs (thought_output_id));
diesel::joinable!(comments -> users (author_id));
diesel::joinable!(sessions -> users (user_id));
diesel::joinable!(thought_inputs -> users (input_user_id));
diesel::joinable!(thought_outputs -> users (author_id));

diesel::allow_tables_to_appear_in_same_query!(
    comments,
    sessions,
    thought_inputs,
    thought_outputs,
    users,
);
