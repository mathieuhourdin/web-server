-- Your SQL goes here
UPDATE thought_outputs SET interaction_type = 'outp';
UPDATE thought_inputs SET resource_maturing_state = 'fnsh';
UPDATE thought_inputs SET resource_publishing_state = 'pbsh';

INSERT INTO thought_outputs (id, resource_title, resource_subtitle, resource_content, resource_comment, interaction_user_id, interaction_progress, resource_maturing_state, resource_publishing_state, resource_parent_id, resource_external_content_url, resource_image_url, created_at, updated_at, resource_type, resource_category_id, interaction_comment, interaction_date, interaction_type, interaction_is_public)
SELECT id, resource_title, resource_subtitle, resource_content, resource_comment, interaction_user_id, interaction_progress, resource_maturing_state, resource_publishing_state, NULL, resource_external_content_url, resource_image_url, created_at, updated_at, resource_type, resource_category_id, interaction_comment, interaction_date, 'inpt', interaction_is_public
FROM thought_inputs;
