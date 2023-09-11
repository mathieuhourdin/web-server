use uuid::Uuid;
use serde::{Serialize, Deserialize};
use diesel::prelude::*;
use crate::db;
use chrono::NaiveDateTime;
use crate::schema::{comments, users};
use crate::http::{HttpRequest, HttpResponse};
use crate::entities::{error::PpdcError, user::User};
use serde_json;

#[derive(Serialize, Deserialize, Queryable, Selectable)]
#[diesel(table_name=comments)]
pub struct Comment {
    pub id: Uuid,
    pub content: String,
    pub thought_output_id: Uuid,
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
    pub thought_output_id: Option<Uuid>,
    pub comment_type: Option<String>,
    pub start_index: Option<i32>,
    pub end_index: Option<i32>,
    pub parent_id: Option<Uuid>,
    pub editing: Option<bool>,
    pub author_id: Option<Uuid>,
}


impl Comment {

    pub fn find_all_for_thought_output(thought_output_id: Uuid) -> Result<Vec<Comment>, PpdcError> {
        let mut conn = db::establish_connection();

        let comments = comments::table
            .filter(comments::thought_output_id.eq(thought_output_id))
            .load::<Comment>(&mut conn)?;
        Ok(comments)
    }

    pub fn find_all_for_thought_output_with_author(thought_output_id: Uuid) -> Result<Vec<CommentWithAuthor>, PpdcError> {
        let mut conn = db::establish_connection();

        let comments = comments::table
            .inner_join(users::table)
            .filter(comments::thought_output_id.eq(thought_output_id))
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

pub fn get_comments_for_thought_output(thought_output_id: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let thought_output_id = HttpRequest::parse_uuid(thought_output_id)?;
    if request.query.contains_key("author") && request.query["author"] == "true" {
        HttpResponse::ok()
            .json(&Comment::find_all_for_thought_output_with_author(thought_output_id)?)
    } else {
        HttpResponse::ok()
            .json(&Comment::find_all_for_thought_output(thought_output_id)?)
    }
}

pub fn post_comment_route(id: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {

    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }
    let id = HttpRequest::parse_uuid(id)?;
    let mut comment = serde_json::from_str::<NewComment>(&request.body[..])?;
    comment.author_id = request.session.as_ref().unwrap().user_id;
    comment.thought_output_id = Some(id);
    let comment = Comment::create(comment)?;
    HttpResponse::ok()
        .json(&comment)
}

pub fn put_comment(id: &str, request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    if request.session.as_ref().unwrap().user_id.is_none() {
        return Ok(HttpResponse::unauthorized());
    }
    let id = HttpRequest::parse_uuid(id)?;
    let db_comment = Comment::find(id)?;
    let comment = serde_json::from_str::<NewComment>(&request.body[..])?;
    if db_comment.author_id != request.session.as_ref().unwrap().user_id.unwrap() 
        && &db_comment.content != comment.content.as_ref().unwrap_or(&"".to_string())
    {
        return Ok(HttpResponse::unauthorized());
    }
    HttpResponse::ok()
        .json(&Comment::update(&id, comment)?)
}
