use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Bool, Text, Uuid as SqlUuid};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    asset::Asset,
    error::PpdcError,
    usage_event::{create_anonymous_lifecycle_usage_event, UsageEventType},
};
use crate::schema::users;

use super::model::User;

#[derive(diesel::QueryableByName)]
struct ExistsRow {
    #[diesel(sql_type = Bool)]
    exists: bool,
}

fn table_exists(
    table_name: &str,
    conn: &mut diesel::PgConnection,
) -> diesel::QueryResult<bool> {
    Ok(sql_query("SELECT to_regclass($1) IS NOT NULL AS exists")
        .bind::<Text, _>(table_name)
        .get_result::<ExistsRow>(conn)?
        .exists)
}

/// Permanently removes an account and all user-linked data.
///
/// Storage objects are removed before the database transaction. A storage error stops the
/// operation before database data is removed.
pub async fn purge_user(
    user_id: Uuid,
    deletion_kind: &'static str,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let user = User::find(&user_id, pool)?;
    let assets = Asset::find_all_owned_by(user_id, pool)?;

    for asset in &assets {
        asset.delete_storage_objects().await?;
    }

    let mut conn = pool.get()?;
    conn.transaction::<_, diesel::result::Error, _>(|conn| {
        // These tables intentionally retain no data that can contain the deleted user's content.
        sql_query(
            "DELETE FROM content_reports WHERE reporter_user_id = $1 OR reported_user_id = $1 OR reviewed_by_user_id = $1",
        )
        .bind::<SqlUuid, _>(user_id)
        .execute(conn)?;
        sql_query(
            r#"
            DELETE FROM outbound_emails
            WHERE recipient_user_id = $1
               OR lower(to_email) = lower($2)
               OR resource_id IN (
                   SELECT id FROM posts WHERE user_id = $1
                   UNION SELECT id FROM traces WHERE user_id = $1
                   UNION SELECT id FROM documents WHERE owner_user_id = $1
                   UNION SELECT id FROM albums WHERE owner_user_id = $1
                   UNION SELECT id FROM messages WHERE sender_user_id = $1 OR recipient_user_id = $1
                   UNION SELECT id FROM landscape_analyses WHERE user_id = $1
               )
            "#,
        )
        .bind::<SqlUuid, _>(user_id)
        .bind::<Text, _>(&user.email)
        .execute(conn)?;
        // LLM calls have an ON DELETE SET NULL analysis FK; delete them explicitly because their
        // prompts and responses can contain the account's journal material.
        sql_query(
            "DELETE FROM llm_calls WHERE analysis_id IN (SELECT id FROM landscape_analyses WHERE user_id = $1)",
        )
        .bind::<SqlUuid, _>(user_id)
        .execute(conn)?;
        // Linked usage events are deleted; anonymous lifecycle events have user_id NULL.
        sql_query("DELETE FROM usage_events WHERE user_id = $1")
            .bind::<SqlUuid, _>(user_id)
            .execute(conn)?;
        sql_query("DELETE FROM sessions WHERE user_id = $1")
            .bind::<SqlUuid, _>(user_id)
            .execute(conn)?;

        // Legacy tables are optional: current v2 databases may already have dropped them.
        if table_exists("comments", conn)? {
            sql_query(
                "UPDATE comments SET parent_id = NULL WHERE parent_id IN (SELECT id FROM comments WHERE author_id = $1)",
            )
            .bind::<SqlUuid, _>(user_id)
            .execute(conn)?;
            sql_query("DELETE FROM comments WHERE author_id = $1")
                .bind::<SqlUuid, _>(user_id)
                .execute(conn)?;
        }
        if table_exists("thought_inputs", conn)? {
            sql_query("DELETE FROM thought_inputs WHERE input_user_id = $1")
                .bind::<SqlUuid, _>(user_id)
                .execute(conn)?;
        }
        if table_exists("interactions", conn)?
            && table_exists("resources", conn)?
            && table_exists("resource_relations", conn)?
            && table_exists("comments", conn)?
        {
            sql_query(
                r#"
                WITH owned_interactions AS (
                    SELECT id, resource_id FROM interactions WHERE interaction_user_id = $1
                ),
                deleted_relations AS (
                    DELETE FROM resource_relations
                    WHERE user_id = $1
                       OR origin_resource_id IN (SELECT resource_id FROM owned_interactions)
                       OR target_resource_id IN (SELECT resource_id FROM owned_interactions)
                ),
                deleted_comments AS (
                    DELETE FROM comments
                    WHERE thought_output_id IN (SELECT id FROM owned_interactions)
                ),
                deleted_interactions AS (
                    DELETE FROM interactions WHERE interaction_user_id = $1
                )
                DELETE FROM resources
                WHERE id IN (SELECT resource_id FROM owned_interactions)
                "#,
            )
            .bind::<SqlUuid, _>(user_id)
            .execute(conn)?;
        }
        if table_exists("articles", conn)? {
            sql_query(
                "UPDATE articles SET parent_id = NULL WHERE parent_id IN (SELECT id FROM articles WHERE author_id = $1)",
            )
            .bind::<SqlUuid, _>(user_id)
            .execute(conn)?;
            sql_query("DELETE FROM articles WHERE author_id = $1")
                .bind::<SqlUuid, _>(user_id)
                .execute(conn)?;
        }

        diesel::delete(users::table.filter(users::id.eq(user_id))).execute(conn)?;
        Ok(())
    })?;

    let event_type = match deletion_kind {
        "self" => UsageEventType::AccountSelfDeleted,
        "admin" => UsageEventType::AccountAdminDeleted,
        _ => UsageEventType::AccountDeleted,
    };
    create_anonymous_lifecycle_usage_event(event_type, None, pool)?;
    Ok(())
}
