use super::{hub, Hub, Notice, Subscription, Wake};
use crate::{
    db::DbPool,
    entities_v2::{
        error::{ErrorType, PpdcError},
        message::{Message, MessageReaction},
        session::Session,
    },
};
use axum::{
    extract::Extension,
    http::{header, HeaderValue},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Bool, Uuid as SqlUuid},
};
use futures_util::stream::unfold;
use serde_json::{json, Value};
use std::{
    convert::Infallible,
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use tokio::{
    sync::broadcast::error::RecvError,
    time::{interval_at, Instant, MissedTickBehavior},
};
use uuid::Uuid;

fn error(status: u32, text: &str) -> PpdcError {
    PpdcError::new(
        status,
        if status >= 500 {
            ErrorType::InternalError
        } else {
            ErrorType::ApiError
        },
        text.into(),
    )
}
#[derive(QueryableByName)]
struct Flag {
    #[diesel(sql_type=Bool)]
    value: bool,
}

pub(super) struct Outbound {
    pub(super) name: &'static str,
    pub(super) data: Value,
}
impl Outbound {
    fn event(self) -> Event {
        // Data is a serde_json::Value, whose serialization cannot fail.
        Event::default()
            .event(self.name)
            .data(self.data.to_string())
    }
    fn resync(reason: &'static str) -> Self {
        Self {
            name: "resync_required",
            data: json!({"reason":reason}),
        }
    }
}
pub(super) enum Projection {
    Event(Outbound),
    Skip,
    Unauthorized,
}

fn session_valid(user: Uuid, session: Uuid, conn: &mut PgConnection) -> Result<bool, PpdcError> {
    Ok(diesel::sql_query("SELECT EXISTS(SELECT 1 FROM sessions WHERE id=$1 AND user_id=$2
        AND authenticated=TRUE AND revoked_at IS NULL AND expires_at>timezone('UTC',now())) AS value")
        .bind::<SqlUuid,_>(session).bind::<SqlUuid,_>(user).get_result::<Flag>(conn)?.value)
}

pub(super) fn project(
    user: Uuid,
    session: Uuid,
    notice: &Notice,
    pool: &DbPool,
) -> Result<Projection, PpdcError> {
    let mut conn = pool.get_timeout(Duration::from_secs(2))?;
    conn.transaction::<_,PpdcError,_>(|conn| {
        if !session_valid(user,session,conn)? {return Ok(Projection::Unauthorized);}
        match *notice {
            Notice::AccessChanged{user_id,other_user_id}=>return Ok(if user==user_id||user==other_user_id {
                Projection::Event(Outbound::resync("access_changed"))
            }else{Projection::Skip}),
            Notice::MemberStateChanged{user_id,..} if user_id!=user=>return Ok(Projection::Skip),
            _=>{},
        }
        let conversation=notice.conversation_id().expect("conversation notice");
        if let Some((id,position))=notice.message_ref() {
            if matches!(notice,Notice::MessageDeleted{..}) {
                // The row no longer exists, so the normal ID-based helper cannot
                // be used. Apply its canonical conversation ACL and history bound.
                let allowed=diesel::sql_query("SELECT EXISTS(SELECT 1 FROM conversation_members cm
                    WHERE cm.conversation_id=$1 AND cm.user_id=$2 AND cm.left_at IS NULL
                    AND cm.history_start_position<=$3 AND conversation_readable($1,$2)) AS value")
                    .bind::<SqlUuid,_>(conversation).bind::<SqlUuid,_>(user).bind::<BigInt,_>(position)
                    .get_result::<Flag>(conn)?.value;
                return Ok(if allowed {
                    Projection::Event(Outbound{name:"message_deleted",data:json!({"conversation_id":conversation,"message_id":id,"position":position})})
                }else{Projection::Skip});
            }
            match crate::entities_v2::social::conversation::service::ensure_message_readable_on_connection(id,user,conn) {
                Ok(())=>{},
                Err(err) if err.status_code==403=>return Ok(Projection::Skip),
                Err(err)=>return Err(err),
            }
            let messages=Message::find_many_with_conn(&[id],conn)?;
            let Some(message)=messages.first() else {return Ok(Projection::Skip);};
            if message.conversation_id!=conversation || message.position!=position {return Ok(Projection::Skip);}
            let message=MessageReaction::hydrate_messages_with_conn(user,messages,conn)?.pop().expect("one message");
            let name=match notice {
                Notice::MessageCreated{..}=>"message_created",
                Notice::ReactionUpdated{..}=>"reaction_updated",
                _=>"message_updated",
            };
            return Ok(Projection::Event(Outbound{name,data:json!({"conversation_id":conversation,"message":message})}));
        }
        let readable=diesel::sql_query("SELECT conversation_readable($1,$2) AS value")
            .bind::<SqlUuid,_>(conversation).bind::<SqlUuid,_>(user).get_result::<Flag>(conn)?.value;
        if readable {
            Ok(Projection::Event(Outbound{name:"conversation_updated",data:json!({"conversation_id":conversation})}))
        } else if matches!(notice,Notice::MembershipChanged{user_id,..} if *user_id==user) {
            // Only the affected member is told about losing their own conversation.
            // Never include group titles, other members or an inaccessible message.
            Ok(Projection::Event(Outbound{name:"conversation_removed",data:json!({"conversation_id":conversation})}))
        } else {Ok(Projection::Skip)}
    })
}

pub async fn stream(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
) -> Result<Response, PpdcError> {
    stream_with_hub(pool, session, hub()).await
}

pub(super) async fn stream_with_hub(
    pool: DbPool,
    session: Session,
    hub: Arc<Hub>,
) -> Result<Response, PpdcError> {
    let user = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let session_id = session.id;
    let p = pool.clone();
    let valid = tokio::task::spawn_blocking(move || {
        session_valid(
            user,
            session_id,
            &mut *p.get_timeout(Duration::from_secs(2))?,
        )
    })
    .await
    .map_err(|_| error(500, "Conversation stream authentication failed"))??;
    if !valid {
        return Err(PpdcError::unauthorized());
    }
    if !hub.ready.load(Ordering::SeqCst) {
        return Err(error(
            503,
            "Conversation event listener is reconnecting; retry shortly",
        ));
    }
    let subscription = hub
        .subscribe(user)
        .ok_or_else(|| error(429, "Too many active conversation streams"))?;
    if !hub.ready.load(Ordering::SeqCst) {
        return Err(error(
            503,
            "Conversation event listener is reconnecting; retry shortly",
        ));
    }
    let mut timer = interval_at(
        Instant::now() + Duration::from_secs(30),
        Duration::from_secs(30),
    );
    timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
    struct State {
        subscription: Subscription,
        pool: DbPool,
        session: Uuid,
        timer: tokio::time::Interval,
        initial: bool,
        done: bool,
    }
    let state = State {
        subscription,
        pool,
        session: session_id,
        timer,
        initial: true,
        done: false,
    };
    let events = unfold(state, |mut state| async move {
        if state.done {
            return None;
        }
        if state.initial {
            state.initial = false;
            return Some((
                Ok::<_, Infallible>(Outbound::resync("connected").event()),
                state,
            ));
        }
        loop {
            let notice = tokio::select! {
                wake=state.subscription.receiver.recv()=>match wake {
                    Ok(Wake::Change(notice))=>Some(notice),
                    Ok(Wake::Resync(reason))=>return Some((Ok(Outbound::resync(reason).event()),state)),
                    Err(RecvError::Lagged(_))=>return Some((Ok(Outbound::resync("client_lagged").event()),state)),
                    Err(RecvError::Closed)=>{state.done=true;return Some((Ok(Outbound::resync("stream_closed").event()),state));},
                },
                _=state.timer.tick()=>None,
            };
            let pool = state.pool.clone();
            let user = state.subscription.user;
            let session = state.session;
            let result = tokio::task::spawn_blocking(move || {
                if let Some(notice) = notice {
                    project(user, session, &notice, &pool)
                } else {
                    let valid = session_valid(
                        user,
                        session,
                        &mut *pool.get_timeout(Duration::from_secs(2))?,
                    )?;
                    Ok(if valid {
                        Projection::Skip
                    } else {
                        Projection::Unauthorized
                    })
                }
            })
            .await;
            match result {
                Ok(Ok(Projection::Event(outbound))) => return Some((Ok(outbound.event()), state)),
                Ok(Ok(Projection::Skip)) => continue,
                Ok(Ok(Projection::Unauthorized)) => {
                    state.done = true;
                    return Some((
                        Ok(Outbound {
                            name: "authentication_expired",
                            data: json!({}),
                        }
                        .event()),
                        state,
                    ));
                }
                _ => {
                    tracing::warn!(target:"conversation_events",user_id=%user,session_id=%session,"conversation_event_projection_failed");
                    state.done = true;
                    return Some((Ok(Outbound::resync("projection_failed").event()), state));
                }
            }
        }
    });
    let mut response = Sse::new(events)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
        .headers_mut()
        .insert("x-accel-buffering", HeaderValue::from_static("no"));
    Ok(response)
}
