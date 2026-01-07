use crate::db;
use crate::entities::{error::PpdcError, session::Session, user::User};
use crate::schema::{comments, users};
use axum::{
    debug_handler,
    extract::{Extension, Json, Path},
};
use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Queryable, Selectable)]
#[diesel(table_name=comments)]
pub struct Comment {
    pub id: Uuid,
    pub content: String,
    pub resource_id: Uuid,
    pub comment_type: Option<String>,
    pub start_index: Option<i32>,
    pub end_index: Option<i32>,
    pub parent_id: Option<Uuid>,
    pub editing: bool,
    pub author_id: Uuid,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize, Queryable)]
pub struct CommentWithAuthor {
    #[serde(flatten)]
    pub comment: Comment,
    pub author: User,
}

#[derive(Deserialize, Insertable, Queryable, AsChangeset)]
#[diesel(table_name=comments)]
pub struct NewComment {
    pub content: Option<String>,
    pub resource_id: Option<Uuid>,
    pub comment_type: Option<String>,
    pub start_index: Option<i32>,
    pub end_index: Option<i32>,
    pub parent_id: Option<Uuid>,
    pub editing: Option<bool>,
    pub author_id: Option<Uuid>,
}

impl Comment {
    pub fn find_all_for_resource(resource_id: Uuid) -> Result<Vec<Comment>, PpdcError> {
        let mut conn = db::establish_connection();

        let comments = comments::table
            .filter(comments::resource_id.eq(resource_id))
            .load::<Comment>(&mut conn)?;
        Ok(comments)
    }

    pub fn find_all_for_resource_with_author(
        resource_id: Uuid,
    ) -> Result<Vec<CommentWithAuthor>, PpdcError> {
        let mut conn = db::establish_connection();

        let comments = comments::table
            .inner_join(users::table)
            .filter(comments::resource_id.eq(resource_id))
            .select((Comment::as_select(), User::as_select()))
            .load::<(Comment, User)>(&mut conn)?
            .into_iter()
            .map(|(comment, author)| CommentWithAuthor { comment, author })
            .collect();
        Ok(comments)
    }

    pub fn find(id: Uuid) -> Result<Comment, PpdcError> {
        let mut conn = db::establish_connection();

        let comment = comments::table
            .filter(comments::id.eq(id))
            .first(&mut conn)?;
        Ok(comment)
    }

    pub fn create(comment: NewComment) -> Result<Comment, PpdcError> {
        let mut conn = db::establish_connection();

        let comment = diesel::insert_into(comments::table)
            .values(&comment)
            .get_result(&mut conn)?;
        Ok(comment)
    }

    pub fn update(id: &Uuid, comment: NewComment) -> Result<Comment, PpdcError> {
        let mut conn = db::establish_connection();

        let comment = diesel::update(comments::table)
            .filter(comments::id.eq(id))
            .set(&comment)
            .get_result(&mut conn)?;
        Ok(comment)
    }
}

#[debug_handler]
pub async fn get_comments_for_resource(
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<CommentWithAuthor>>, PpdcError> {
    Ok(Json(Comment::find_all_for_resource_with_author(id)?))
}

#[debug_handler]
pub async fn post_comment_route(
    Path(id): Path<Uuid>,
    Extension(session): Extension<Session>,
    Json(mut payload): Json<NewComment>,
) -> Result<Json<Comment>, PpdcError> {
    payload.author_id = session.user_id;
    payload.resource_id = Some(id);
    Ok(Json(Comment::create(payload)?))
}

#[debug_handler]
pub async fn put_comment(
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(payload): Json<NewComment>,
) -> Result<Json<Comment>, PpdcError> {
    let db_comment = Comment::find(id)?;
    if db_comment.author_id != session.user_id.unwrap()
        && &db_comment.content != payload.content.as_ref().unwrap_or(&"".to_string())
    {
        return Err(PpdcError::unauthorized());
    }
    Ok(Json(Comment::update(&id, payload)?))
}
