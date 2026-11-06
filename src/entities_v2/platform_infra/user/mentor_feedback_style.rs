use crate::entities_v2::error::{ErrorType, PpdcError};
use diesel::{
    deserialize::{self, FromSql},
    pg::{Pg, PgValue},
    serialize::{self, Output, ToSql},
    sql_types::Text,
    AsExpression, FromSqlRow,
};
use serde::{Deserialize, Serialize};

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, AsExpression, FromSqlRow,
)]
#[serde(rename_all = "snake_case")]
#[diesel(sql_type = Text)]
pub enum MentorFeedbackStyle {
    #[default]
    Balanced,
    Challenging,
    Companionable,
}

impl MentorFeedbackStyle {
    pub fn to_db(self) -> &'static str {
        match self {
            Self::Balanced => "BALANCED",
            Self::Challenging => "CHALLENGING",
            Self::Companionable => "COMPANIONABLE",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, PpdcError> {
        match value {
            "BALANCED" => Ok(Self::Balanced),
            "CHALLENGING" => Ok(Self::Challenging),
            "COMPANIONABLE" => Ok(Self::Companionable),
            _ => Err(PpdcError::new(
                500,
                ErrorType::InternalError,
                "Invalid stored mentor_feedback_style".into(),
            )),
        }
    }
}

impl ToSql<Text, Pg> for MentorFeedbackStyle {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        <str as ToSql<Text, Pg>>::to_sql(self.to_db(), out)
    }
}

impl FromSql<Text, Pg> for MentorFeedbackStyle {
    fn from_sql(value: PgValue<'_>) -> deserialize::Result<Self> {
        Self::from_db(&<String as FromSql<Text, Pg>>::from_sql(value)?)
            .map_err(|_| "Invalid stored mentor_feedback_style".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mentor_feedback_styles_have_explicit_api_and_db_values() {
        assert_eq!(
            MentorFeedbackStyle::default(),
            MentorFeedbackStyle::Balanced
        );
        for (style, api, db) in [
            (MentorFeedbackStyle::Balanced, "balanced", "BALANCED"),
            (
                MentorFeedbackStyle::Challenging,
                "challenging",
                "CHALLENGING",
            ),
            (
                MentorFeedbackStyle::Companionable,
                "companionable",
                "COMPANIONABLE",
            ),
        ] {
            assert_eq!(style.to_db(), db);
            assert_eq!(MentorFeedbackStyle::from_db(db).unwrap(), style);
            assert_eq!(serde_json::to_value(style).unwrap(), api);
            assert_eq!(
                serde_json::from_value::<MentorFeedbackStyle>(serde_json::json!(api)).unwrap(),
                style
            );
        }
        assert!(serde_json::from_str::<MentorFeedbackStyle>("\"supportive\"").is_err());
    }

    #[tokio::test]
    #[ignore = "Requires explicitly configured disposable blog_test; synthetic fixtures only, no LLM calls"]
    async fn mentor_feedback_style_defaults_updates_privacy_and_prompt_context() {
        use super::super::routes::{patch_user_route, put_user_route, PatchUserDto};
        use crate::{
            db::DbPool,
            entities_v2::{
                message::{NewMessage, NewMessageDto},
                session::Session,
                user::{NewUser, User, UserPseudonymizedAuthentifiedResponse, UserPublicResponse},
            },
            schema::users,
            work_analyzer::{
                mentor_style::with_mentor_feedback_style, message_processing::context,
            },
        };
        use axum::extract::{Extension, Json, Path};
        use diesel::{connection::SimpleConnection, prelude::*, r2d2::ConnectionManager};
        use diesel_migrations::{embed_migrations, MigrationHarness};
        use uuid::Uuid;

        let url = std::env::var("BLOG_TEST_DATABASE_URL").unwrap();
        assert_eq!(url::Url::parse(&url).unwrap().path(), "/blog_test");
        let pool = DbPool::builder()
            .max_size(8)
            .build(ConnectionManager::<PgConnection>::new(url))
            .unwrap();
        const MIGRATIONS: diesel_migrations::EmbeddedMigrations = embed_migrations!();
        let mut conn = pool.get().unwrap();
        conn.run_pending_migrations(MIGRATIONS).unwrap();
        while conn
            .applied_migrations()
            .unwrap()
            .iter()
            .any(|version| version.to_string().as_str() >= "20261106100000")
        {
            conn.revert_last_migration(MIGRATIONS).unwrap();
        }
        let (a, b, mentor) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        conn.batch_execute(&format!(
            "INSERT INTO users(id,email,first_name,last_name,handle,password)
            SELECT id,id::text||'@test.invalid','Synthetic','Mentor style',id::text,'unused'
            FROM unnest(ARRAY['{a}'::uuid,'{b}','{mentor}']) id;
            UPDATE users SET principal_type='SERVICE',is_platform_user=FALSE WHERE id='{mentor}';
            UPDATE users SET mentor_id='{mentor}' WHERE id='{a}';"
        ))
        .unwrap();
        conn.run_pending_migrations(MIGRATIONS).unwrap();
        drop(conn);
        assert_eq!(
            User::find(&a, &pool).unwrap().mentor_feedback_style,
            MentorFeedbackStyle::Balanced
        );
        let mut session = Session::anonymous();
        session.user_id = Some(a);
        let patch =
            |value: serde_json::Value| serde_json::from_value::<PatchUserDto>(value).unwrap();
        let updated = patch_user_route(
            Extension(pool.clone()),
            Extension(session.clone()),
            Path(a),
            Json(patch(
                serde_json::json!({"mentor_feedback_style":"challenging"}),
            )),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(
            updated.mentor_feedback_style,
            MentorFeedbackStyle::Challenging
        );
        assert!(!updated.allows_ai_features()); // Style never changes consent.
        let updated = patch_user_route(
            Extension(pool.clone()),
            Extension(session.clone()),
            Path(a),
            Json(patch(serde_json::json!({"first_name":"Still synthetic"}))),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(
            updated.mentor_feedback_style,
            MentorFeedbackStyle::Challenging
        );
        let owner = serde_json::to_value(UserPseudonymizedAuthentifiedResponse::from_viewer(
            &updated,
            Some(a),
        ))
        .unwrap();
        assert_eq!(owner["mentor_feedback_style"], "challenging");
        assert!(serde_json::to_value(UserPublicResponse::from(&updated))
            .unwrap()
            .get("mentor_feedback_style")
            .is_none());
        assert!(
            serde_json::to_value(UserPseudonymizedAuthentifiedResponse::from_viewer(
                &updated,
                Some(b)
            ))
            .unwrap()
            .get("mentor_feedback_style")
            .is_none()
        );
        assert!(patch_user_route(
            Extension(pool.clone()),
            Extension(session.clone()),
            Path(b),
            Json(patch(
                serde_json::json!({"mentor_feedback_style":"challenging"})
            ))
        )
        .await
        .is_err());
        let stored: String = users::table
            .filter(users::id.eq(a))
            .select(users::mentor_feedback_style)
            .first(&mut *pool.get().unwrap())
            .unwrap();
        assert_eq!(stored, "CHALLENGING");
        let new_message = |sender, recipient, content: &str| {
            let dto: NewMessageDto = serde_json::from_value(
                serde_json::json!({"recipient_user_id":recipient,"content":content}),
            )
            .unwrap();
            NewMessage::new(dto, sender).create(&pool).unwrap()
        };
        let question = new_message(a, mentor, "Be gentle for this particular reply.");
        let reply = new_message(mentor, a, "Synthetic placeholder");
        let mentor_user = User::find(&mentor, &pool).unwrap();
        let prompt =
            context::build(&reply, &question, None, &mentor_user, &updated, &pool).unwrap();
        assert_eq!(
            prompt.mentor_feedback_style,
            MentorFeedbackStyle::Challenging
        );
        assert_eq!(
            prompt.user_question.content,
            "Be gentle for this particular reply."
        );
        assert!(
            with_mentor_feedback_style("Task", prompt.mentor_feedback_style)
                .contains("explicit current request takes precedence")
        );
        // The existing PUT payload supports the preference, too, and a later
        // legacy PUT omitting it must not restore balanced.
        let payload = |style: Option<&str>| {
            let mut value = serde_json::json!({"email":updated.email,"first_name":updated.first_name,
                "last_name":updated.last_name,"handle":updated.handle});
            if let Some(style) = style {
                value["mentor_feedback_style"] = style.into();
            }
            serde_json::from_value::<NewUser>(value).unwrap()
        };
        let _ = put_user_route(
            Extension(pool.clone()),
            Extension(session.clone()),
            Path(a),
            Json(payload(Some("companionable"))),
        )
        .await
        .unwrap();
        let _ = put_user_route(
            Extension(pool.clone()),
            Extension(session),
            Path(a),
            Json(payload(None)),
        )
        .await
        .unwrap();
        assert_eq!(
            User::find(&a, &pool).unwrap().mentor_feedback_style,
            MentorFeedbackStyle::Companionable
        );
        let mut conn = pool.get().unwrap();
        let invalid = conn.transaction::<(), diesel::result::Error, _>(|conn| {
            conn.batch_execute(&format!(
                "UPDATE users SET mentor_feedback_style='UNKNOWN' WHERE id='{a}'"
            ))
        });
        assert!(invalid.is_err());
        // Exact synthetic IDs only; no user content or external services are used.
        diesel::delete(users::table.filter(users::id.eq_any([a, b, mentor])))
            .execute(&mut conn)
            .unwrap();
    }
}
