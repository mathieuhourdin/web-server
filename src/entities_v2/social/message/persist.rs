use diesel::prelude::*;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::error::PpdcError;
use crate::schema::messages;

use super::model::{Message, NewMessage};

impl Message {
    pub fn update(self, pool: &DbPool) -> Result<Message, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        diesel::update(messages::table.filter(messages::id.eq(self.id)))
            .set((
                messages::recipient_user_id.eq(self.recipient_user_id),
                messages::landscape_analysis_id.eq(self.landscape_analysis_id),
                messages::trace_id.eq(self.trace_id),
                messages::message_type.eq(self.message_type.to_db()),
                messages::title.eq(self.title),
                messages::content.eq(self.content),
            ))
            .execute(&mut conn)?;
        Message::find(self.id, pool)
    }
}

impl NewMessage {
    pub fn create(self, pool: &DbPool) -> Result<Message, PpdcError> {
        let mut conn = pool
            .get()
            .expect("Failed to get a connection from the pool");
        let id: Uuid = diesel::insert_into(messages::table)
            .values((
                messages::sender_user_id.eq(self.sender_user_id),
                messages::recipient_user_id.eq(self.recipient_user_id),
                messages::landscape_analysis_id.eq(self.landscape_analysis_id),
                messages::trace_id.eq(self.trace_id),
                messages::message_type.eq(self.message_type.to_db()),
                messages::title.eq(self.title),
                messages::content.eq(self.content),
            ))
            .returning(messages::id)
            .get_result(&mut conn)?;
        Message::find(id, pool)
    }
}
