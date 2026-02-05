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
        resource_id -> Uuid,
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
        interaction_user_id -> Uuid,
        interaction_progress -> Int4,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        interaction_comment -> Nullable<Text>,
        interaction_date -> Timestamp,
        interaction_type -> Nullable<Text>,
        interaction_is_public -> Bool,
        resource_id -> Nullable<Uuid>,
    }
}

diesel::table! {
    llm_calls (id) {
        id -> Uuid,
        status -> Text,
        model -> Text,
        prompt -> Text,
        schema -> Text,
        request -> Text,
        request_url -> Text,
        response -> Text,
        output -> Text,
        input_tokens_used -> Int4,
        reasoning_tokens_used -> Int4,
        output_tokens_used -> Int4,
        price -> Float8,
        currency -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        analysis_id -> Nullable<Uuid>,
    }
}

diesel::table! {
    resource_relations (id) {
        id -> Uuid,
        origin_resource_id -> Uuid,
        target_resource_id -> Uuid,
        relation_comment -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        user_id -> Uuid,
        relation_type -> Text,
    }
}

diesel::table! {
    resources (id) {
        id -> Uuid,
        title -> Text,
        subtitle -> Text,
        content -> Text,
        external_content_url -> Nullable<Text>,
        comment -> Nullable<Text>,
        image_url -> Nullable<Text>,
        resource_type -> Text,
        maturing_state -> Text,
        publishing_state -> Text,
        category_id -> Nullable<Uuid>,
        is_external -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        entity_type -> Text,
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
        profile_picture_url -> Nullable<Text>,
        is_platform_user -> Bool,
        biography -> Nullable<Text>,
        pseudonym -> Text,
        pseudonymized -> Bool,
    }
}

diesel::joinable!(comments -> resources (resource_id));
diesel::joinable!(comments -> users (author_id));
diesel::joinable!(interactions -> resources (resource_id));
diesel::joinable!(interactions -> users (interaction_user_id));
diesel::joinable!(llm_calls -> resources (analysis_id));
diesel::joinable!(resource_relations -> users (user_id));
diesel::joinable!(resources -> categories (category_id));
diesel::joinable!(sessions -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    categories,
    comments,
    interactions,
    llm_calls,
    resource_relations,
    resources,
    sessions,
    users,
);
