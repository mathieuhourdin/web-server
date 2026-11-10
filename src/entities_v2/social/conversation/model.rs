use crate::{
    entities_v2::{
        error::{ErrorType, PpdcError},
        message::Message,
        user::UserPublicResponse,
    },
    pagination::PaginationParams,
};
use chrono::NaiveDateTime;
use diesel::{
    prelude::*,
    sql_types::{BigInt, Nullable, Text, Timestamp, Uuid as SqlUuid},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConversationKind {
    Direct,
    Group,
}
impl ConversationKind {
    pub fn to_db(self) -> &'static str {
        match self {
            Self::Direct => "DIRECT",
            Self::Group => "GROUP",
        }
    }
    pub fn from_db(s: &str) -> Result<Self, PpdcError> {
        match s {
            "DIRECT" => Ok(Self::Direct),
            "GROUP" => Ok(Self::Group),
            _ => Err(PpdcError::new(
                500,
                ErrorType::InternalError,
                "Invalid conversation kind".into(),
            )),
        }
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    Admin,
    Member,
}
impl MemberRole {
    pub fn to_db(self) -> &'static str {
        match self {
            Self::Admin => "ADMIN",
            Self::Member => "MEMBER",
        }
    }
    pub fn from_db(s: &str) -> Result<Self, PpdcError> {
        match s {
            "ADMIN" => Ok(Self::Admin),
            "MEMBER" => Ok(Self::Member),
            _ => Err(PpdcError::new(
                500,
                ErrorType::InternalError,
                "Invalid conversation role".into(),
            )),
        }
    }
}
#[derive(Clone, Debug, QueryableByName)]
pub(super) struct ConversationRow {
    #[diesel(sql_type=SqlUuid)]
    pub id: Uuid,
    #[diesel(sql_type=Text)]
    pub kind: String,
    #[diesel(sql_type=Nullable<Text>)]
    pub title: Option<String>,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    pub created_by_user_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    pub direct_user_low_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    pub direct_user_high_id: Option<Uuid>,
    #[diesel(sql_type=BigInt)]
    pub last_message_position: i64,
    #[diesel(sql_type=Timestamp)]
    pub created_at: NaiveDateTime,
    #[diesel(sql_type=Timestamp)]
    pub updated_at: NaiveDateTime,
}
#[derive(Clone, Debug, QueryableByName)]
pub(super) struct MembershipRow {
    #[diesel(sql_type=SqlUuid)]
    pub conversation_id: Uuid,
    #[diesel(sql_type=SqlUuid)]
    pub user_id: Uuid,
    #[diesel(sql_type=Text)]
    pub role: String,
    #[diesel(sql_type=Timestamp)]
    pub joined_at: NaiveDateTime,
    #[diesel(sql_type=Nullable<Timestamp>)]
    #[allow(dead_code)]
    pub left_at: Option<NaiveDateTime>,
    #[diesel(sql_type=BigInt)]
    pub history_start_position: i64,
    #[diesel(sql_type=BigInt)]
    pub last_read_position: i64,
    #[diesel(sql_type=Nullable<Timestamp>)]
    pub muted_at: Option<NaiveDateTime>,
    #[diesel(sql_type=Nullable<Text>)]
    pub nickname: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct MembershipView {
    pub role: MemberRole,
    pub joined_at: NaiveDateTime,
    pub history_start_position: i64,
    pub last_read_position: i64,
    pub muted_at: Option<NaiveDateTime>,
    pub nickname: Option<String>,
}
impl TryFrom<MembershipRow> for MembershipView {
    type Error = PpdcError;
    fn try_from(m: MembershipRow) -> Result<Self, Self::Error> {
        Ok(Self {
            role: MemberRole::from_db(&m.role)?,
            joined_at: m.joined_at,
            history_start_position: m.history_start_position,
            last_read_position: m.last_read_position,
            muted_at: m.muted_at,
            nickname: m.nickname,
        })
    }
}
#[derive(Serialize)]
pub struct MemberView {
    pub user: UserPublicResponse,
    pub nickname: Option<String>,
    pub role: MemberRole,
    pub joined_at: NaiveDateTime,
}
#[derive(Serialize)]
pub struct ConversationView {
    pub id: Uuid,
    pub kind: ConversationKind,
    pub title: Option<String>,
    pub created_by_user_id: Option<Uuid>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    /// Viewer-specific ordering date; not a persisted conversation timestamp.
    pub sort_at: NaiveDateTime,
    pub membership: MembershipView,
    pub members_count: i64,
    pub members_preview: Vec<MemberView>,
    pub last_message: Option<Message>,
    pub unread_count: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateConversation {
    pub kind: ConversationKind,
    pub member_user_ids: Vec<Uuid>,
    pub title: Option<String>,
}
#[derive(Deserialize)]
pub struct ConversationsQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub q: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchConversation {
    #[serde(default, deserialize_with = "crate::serde_helpers::double_option")]
    pub title: Option<Option<String>>,
    pub muted: Option<bool>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddMembers {
    pub user_ids: Vec<Uuid>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchMember {
    pub role: Option<MemberRole>,
    #[serde(default, deserialize_with="crate::serde_helpers::double_option")]
    pub nickname: Option<Option<String>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadConversation {
    pub through_position: i64,
}
#[derive(Deserialize)]
pub struct MessagesQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub before_position: Option<i64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendMessage {
    pub content: String,
    #[serde(default)]
    pub image_attachment_ids: Vec<Uuid>,
    pub title: Option<String>,
    pub reply_to_message_id: Option<Uuid>,
    pub trace_id: Option<Uuid>,
    pub post_id: Option<Uuid>,
    pub message_type: Option<crate::entities_v2::message::MessageType>,
    pub attachment_type: Option<crate::entities_v2::message::MessageAttachmentType>,
    pub attachment: Option<crate::entities_v2::message::MessageAttachment>,
    pub writing_thread_id: Option<Uuid>,
    #[serde(alias = "expected_version")]
    pub expected_version_integer: Option<i32>,
    pub landscape_analysis_id: Option<Uuid>,
}
