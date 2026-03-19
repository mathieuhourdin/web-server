// @generated automatically by Diesel CLI.

diesel::table! {
    analysis_summaries (id) {
        id -> Uuid,
        landscape_analysis_id -> Uuid,
        user_id -> Uuid,
        summary_type -> Text,
        title -> Text,
        content -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        short_content -> Text,
        meaningful_event_title -> Nullable<Text>,
        meaningful_event_description -> Nullable<Text>,
        meaningful_event_date -> Nullable<Text>,
    }
}

diesel::table! {
    element_landmarks (id) {
        id -> Uuid,
        element_id -> Uuid,
        landmark_id -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    element_relations (id) {
        id -> Uuid,
        origin_element_id -> Uuid,
        target_element_id -> Uuid,
        relation_type -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    elements (id) {
        id -> Uuid,
        user_id -> Uuid,
        analysis_id -> Uuid,
        trace_id -> Uuid,
        trace_mirror_id -> Nullable<Uuid>,
        title -> Text,
        subtitle -> Text,
        content -> Text,
        extended_content -> Nullable<Text>,
        verb -> Text,
        element_type -> Text,
        element_subtype -> Text,
        interaction_date -> Nullable<Timestamp>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        status -> Nullable<Text>,
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
    journal_grants (id) {
        id -> Uuid,
        journal_id -> Uuid,
        owner_user_id -> Uuid,
        grantee_user_id -> Nullable<Uuid>,
        grantee_scope -> Nullable<Text>,
        access_level -> Text,
        status -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    journals (id) {
        id -> Uuid,
        user_id -> Uuid,
        title -> Text,
        subtitle -> Text,
        content -> Text,
        journal_type -> Text,
        status -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        is_encrypted -> Bool,
        last_trace_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    landmark_relations (id) {
        id -> Uuid,
        origin_landmark_id -> Uuid,
        target_landmark_id -> Uuid,
        relation_type -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    landmarks (id) {
        id -> Uuid,
        analysis_id -> Uuid,
        user_id -> Uuid,
        parent_id -> Nullable<Uuid>,
        title -> Text,
        subtitle -> Text,
        content -> Text,
        external_content_url -> Nullable<Text>,
        comment -> Nullable<Text>,
        image_url -> Nullable<Text>,
        landmark_type -> Text,
        maturing_state -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        related_elements_count -> Int4,
        last_related_element_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    landscape_analyses (id) {
        id -> Uuid,
        user_id -> Uuid,
        title -> Text,
        subtitle -> Text,
        plain_text_state_summary -> Text,
        interaction_date -> Nullable<Timestamp>,
        processing_state -> Text,
        parent_id -> Nullable<Uuid>,
        replayed_from_id -> Nullable<Uuid>,
        analyzed_trace_id -> Nullable<Uuid>,
        trace_mirror_id -> Nullable<Uuid>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        landscape_analysis_type -> Text,
        period_start -> Timestamp,
        period_end -> Timestamp,
    }
}

diesel::table! {
    landscape_analysis_inputs (id) {
        id -> Uuid,
        landscape_analysis_id -> Uuid,
        trace_id -> Nullable<Uuid>,
        trace_mirror_id -> Nullable<Uuid>,
        input_type -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    landscape_landmarks (id) {
        id -> Uuid,
        landscape_analysis_id -> Uuid,
        landmark_id -> Uuid,
        relation_type -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    lens_analysis_scopes (id) {
        id -> Uuid,
        lens_id -> Uuid,
        landscape_analysis_id -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    lens_heads (id) {
        id -> Uuid,
        lens_id -> Uuid,
        landscape_analysis_id -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    lens_targets (id) {
        id -> Uuid,
        lens_id -> Uuid,
        trace_id -> Uuid,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    lenses (id) {
        id -> Uuid,
        user_id -> Uuid,
        processing_state -> Text,
        fork_landscape_id -> Nullable<Uuid>,
        current_landscape_id -> Nullable<Uuid>,
        target_trace_id -> Nullable<Uuid>,
        autoplay -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        run_lock_owner -> Nullable<Uuid>,
        run_lock_until -> Nullable<Timestamp>,
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
        system_prompt -> Text,
        user_prompt -> Text,
        display_name -> Text,
    }
}

diesel::table! {
    messages (id) {
        id -> Uuid,
        sender_user_id -> Uuid,
        recipient_user_id -> Uuid,
        landscape_analysis_id -> Nullable<Uuid>,
        trace_id -> Nullable<Uuid>,
        message_type -> Text,
        title -> Text,
        content -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        reply_to_message_id -> Nullable<Uuid>,
        processing_state -> Text,
        attachment_type -> Nullable<Text>,
        attachment -> Nullable<Jsonb>,
        seen_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    outbound_emails (id) {
        id -> Uuid,
        recipient_user_id -> Nullable<Uuid>,
        reason -> Text,
        resource_type -> Nullable<Text>,
        resource_id -> Nullable<Uuid>,
        to_email -> Text,
        from_email -> Text,
        subject -> Text,
        text_body -> Nullable<Text>,
        html_body -> Nullable<Text>,
        status -> Text,
        provider -> Text,
        provider_message_id -> Nullable<Text>,
        attempt_count -> Int4,
        last_error -> Nullable<Text>,
        scheduled_at -> Nullable<Timestamp>,
        sent_at -> Nullable<Timestamp>,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    post_relations (id) {
        id -> Uuid,
        origin_post_id -> Uuid,
        target_post_id -> Uuid,
        relation_type -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    posts (id) {
        id -> Uuid,
        user_id -> Uuid,
        title -> Text,
        subtitle -> Text,
        content -> Text,
        image_url -> Nullable<Text>,
        interaction_type -> Text,
        post_type -> Text,
        publishing_date -> Nullable<Timestamp>,
        publishing_state -> Text,
        maturing_state -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    references (id) {
        id -> Uuid,
        tag_id -> Int4,
        trace_mirror_id -> Uuid,
        landmark_id -> Nullable<Uuid>,
        landscape_analysis_id -> Uuid,
        user_id -> Uuid,
        mention -> Text,
        reference_type -> Text,
        context_tags -> Jsonb,
        reference_variants -> Jsonb,
        parent_reference_id -> Nullable<Uuid>,
        is_user_specific -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    relationships (id) {
        id -> Uuid,
        requester_user_id -> Uuid,
        target_user_id -> Uuid,
        relationship_type -> Text,
        status -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        accepted_at -> Nullable<Timestamp>,
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
        relation_entity_pair -> Text,
        relation_meaning -> Text,
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
        is_external -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        entity_type -> Text,
        resource_subtype -> Nullable<Text>,
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
        secret_hash -> Nullable<Text>,
        revoked_at -> Nullable<Timestamp>,
        last_seen_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    trace_mirrors (id) {
        id -> Uuid,
        user_id -> Uuid,
        trace_id -> Uuid,
        landscape_analysis_id -> Uuid,
        primary_landmark_id -> Nullable<Uuid>,
        title -> Text,
        subtitle -> Text,
        content -> Text,
        trace_mirror_type -> Text,
        tags -> Jsonb,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    traces (id) {
        id -> Uuid,
        user_id -> Uuid,
        journal_id -> Uuid,
        title -> Text,
        subtitle -> Text,
        content -> Text,
        interaction_date -> Timestamp,
        trace_type -> Text,
        status -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        is_encrypted -> Bool,
        encryption_metadata -> Nullable<Jsonb>,
        start_writing_at -> Timestamp,
        finalized_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    user_roles (id) {
        id -> Uuid,
        user_id -> Uuid,
        role -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    user_secure_actions (id) {
        id -> Uuid,
        user_id -> Uuid,
        action_type -> Text,
        payload -> Nullable<Text>,
        secret_hash -> Text,
        expires_at -> Timestamp,
        used_at -> Nullable<Timestamp>,
        revoked_at -> Nullable<Timestamp>,
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
        high_level_projects_definition -> Nullable<Text>,
        journal_theme -> Text,
        current_lens_id -> Nullable<Uuid>,
        week_analysis_weekday -> Int2,
        timezone -> Text,
        context_anchor_at -> Nullable<Timestamp>,
        principal_type -> Text,
        mentor_id -> Nullable<Uuid>,
        welcome_message -> Nullable<Text>,
    }
}

diesel::joinable!(analysis_summaries -> landscape_analyses (landscape_analysis_id));
diesel::joinable!(analysis_summaries -> users (user_id));
diesel::joinable!(element_landmarks -> elements (element_id));
diesel::joinable!(element_landmarks -> landmarks (landmark_id));
diesel::joinable!(elements -> landscape_analyses (analysis_id));
diesel::joinable!(elements -> trace_mirrors (trace_mirror_id));
diesel::joinable!(elements -> traces (trace_id));
diesel::joinable!(elements -> users (user_id));
diesel::joinable!(interactions -> resources (resource_id));
diesel::joinable!(interactions -> users (interaction_user_id));
diesel::joinable!(journal_grants -> journals (journal_id));
diesel::joinable!(journals -> users (user_id));
diesel::joinable!(landmarks -> landscape_analyses (analysis_id));
diesel::joinable!(landmarks -> users (user_id));
diesel::joinable!(landscape_analyses -> traces (analyzed_trace_id));
diesel::joinable!(landscape_analyses -> users (user_id));
diesel::joinable!(landscape_analysis_inputs -> landscape_analyses (landscape_analysis_id));
diesel::joinable!(landscape_analysis_inputs -> trace_mirrors (trace_mirror_id));
diesel::joinable!(landscape_analysis_inputs -> traces (trace_id));
diesel::joinable!(landscape_landmarks -> landmarks (landmark_id));
diesel::joinable!(landscape_landmarks -> landscape_analyses (landscape_analysis_id));
diesel::joinable!(lens_analysis_scopes -> landscape_analyses (landscape_analysis_id));
diesel::joinable!(lens_analysis_scopes -> lenses (lens_id));
diesel::joinable!(lens_heads -> landscape_analyses (landscape_analysis_id));
diesel::joinable!(lens_heads -> lenses (lens_id));
diesel::joinable!(lens_targets -> lenses (lens_id));
diesel::joinable!(lens_targets -> traces (trace_id));
diesel::joinable!(lenses -> traces (target_trace_id));
diesel::joinable!(lenses -> users (user_id));
diesel::joinable!(llm_calls -> landscape_analyses (analysis_id));
diesel::joinable!(messages -> landscape_analyses (landscape_analysis_id));
diesel::joinable!(messages -> traces (trace_id));
diesel::joinable!(outbound_emails -> users (recipient_user_id));
diesel::joinable!(posts -> users (user_id));
diesel::joinable!(references -> landmarks (landmark_id));
diesel::joinable!(references -> landscape_analyses (landscape_analysis_id));
diesel::joinable!(references -> trace_mirrors (trace_mirror_id));
diesel::joinable!(references -> users (user_id));
diesel::joinable!(resource_relations -> users (user_id));
diesel::joinable!(sessions -> users (user_id));
diesel::joinable!(trace_mirrors -> landmarks (primary_landmark_id));
diesel::joinable!(trace_mirrors -> traces (trace_id));
diesel::joinable!(trace_mirrors -> users (user_id));
diesel::joinable!(traces -> journals (journal_id));
diesel::joinable!(traces -> users (user_id));
diesel::joinable!(user_roles -> users (user_id));
diesel::joinable!(user_secure_actions -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    analysis_summaries,
    element_landmarks,
    element_relations,
    elements,
    interactions,
    journal_grants,
    journals,
    landmark_relations,
    landmarks,
    landscape_analyses,
    landscape_analysis_inputs,
    landscape_landmarks,
    lens_analysis_scopes,
    lens_heads,
    lens_targets,
    lenses,
    llm_calls,
    messages,
    outbound_emails,
    post_relations,
    posts,
    references,
    relationships,
    resource_relations,
    resources,
    sessions,
    trace_mirrors,
    traces,
    user_roles,
    user_secure_actions,
    users,
);
