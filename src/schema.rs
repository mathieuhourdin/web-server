// @generated automatically by Diesel CLI.

diesel::table! {
    categories (id) {
        id -> Uuid,
        display_name -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

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
    interactions (id) {
        id -> Uuid,
        resource_title -> Text,
        resource_subtitle -> Text,
        resource_content -> Text,
        resource_comment -> Text,
        interaction_user_id -> Nullable<Uuid>,
        interaction_progress -> Int4,
        resource_maturing_state -> Text,
        resource_publishing_state -> Text,
        resource_parent_id -> Nullable<Uuid>,
        resource_external_content_url -> Nullable<Text>,
        resource_image_url -> Nullable<Text>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        resource_type -> Text,
        resource_category_id -> Nullable<Uuid>,
        interaction_comment -> Nullable<Text>,
        interaction_date -> Nullable<Timestamp>,
        interaction_type -> Nullable<Text>,
        interaction_is_public -> Bool,
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
    thought_input_usages (thought_input_id, thought_output_id) {
        id -> Uuid,
        thought_input_id -> Uuid,
        thought_output_id -> Uuid,
        usage_reason -> Text,
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

diesel::joinable!(comments -> interactions (thought_output_id));
diesel::joinable!(comments -> users (author_id));
diesel::joinable!(interactions -> categories (resource_category_id));
diesel::joinable!(interactions -> users (interaction_user_id));
diesel::joinable!(sessions -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    categories,
    comments,
    interactions,
    sessions,
    thought_input_usages,
    users,
);
