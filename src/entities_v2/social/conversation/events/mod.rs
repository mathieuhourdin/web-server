//! Best-effort conversation events. PostgreSQL remains the source of truth.
mod listener;
pub mod routes;

use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};
use tokio::sync::broadcast;
use uuid::Uuid;

const CAPACITY: usize = 64;
const MAX_STREAMS_PER_USER: usize = 5;
const MAX_STREAMS: usize = 256;

/// IDs only. This channel is internal, never forwarded without authorization.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Notice {
    MessageCreated {
        conversation_id: Uuid,
        message_id: Uuid,
        position: i64,
    },
    MessageUpdated {
        conversation_id: Uuid,
        message_id: Uuid,
        position: i64,
    },
    MessageDeleted {
        conversation_id: Uuid,
        message_id: Uuid,
        position: i64,
    },
    ReactionUpdated {
        conversation_id: Uuid,
        message_id: Uuid,
        position: i64,
    },
    ConversationChanged {
        conversation_id: Uuid,
    },
    MembershipChanged {
        conversation_id: Uuid,
        user_id: Uuid,
    },
    MemberStateChanged {
        conversation_id: Uuid,
        user_id: Uuid,
    },
    AccessChanged {
        user_id: Uuid,
        other_user_id: Uuid,
    },
}
impl Notice {
    fn conversation_id(&self) -> Option<Uuid> {
        match *self {
            Self::MessageCreated {
                conversation_id, ..
            }
            | Self::MessageUpdated {
                conversation_id, ..
            }
            | Self::MessageDeleted {
                conversation_id, ..
            }
            | Self::ReactionUpdated {
                conversation_id, ..
            }
            | Self::ConversationChanged { conversation_id }
            | Self::MembershipChanged {
                conversation_id, ..
            }
            | Self::MemberStateChanged {
                conversation_id, ..
            } => Some(conversation_id),
            Self::AccessChanged { .. } => None,
        }
    }
    fn message_ref(&self) -> Option<(Uuid, i64)> {
        match *self {
            Self::MessageCreated {
                message_id,
                position,
                ..
            }
            | Self::MessageUpdated {
                message_id,
                position,
                ..
            }
            | Self::MessageDeleted {
                message_id,
                position,
                ..
            }
            | Self::ReactionUpdated {
                message_id,
                position,
                ..
            } => Some((message_id, position)),
            _ => None,
        }
    }
}

#[derive(Clone)]
enum Wake {
    Change(Arc<Notice>),
    Resync(&'static str),
}
struct UserStreams {
    sender: broadcast::Sender<Wake>,
    count: usize,
}
#[derive(Default)]
struct Subscribers {
    users: HashMap<Uuid, UserStreams>,
    total: usize,
}
pub(super) struct Hub {
    subscribers: Mutex<Subscribers>,
    ready: AtomicBool,
    started: AtomicBool,
}
impl Hub {
    fn new() -> Self {
        Self {
            subscribers: Mutex::new(Subscribers::default()),
            ready: AtomicBool::new(false),
            started: AtomicBool::new(false),
        }
    }
    fn subscribe(self: &Arc<Self>, user: Uuid) -> Option<Subscription> {
        let mut subscribers = self
            .subscribers
            .lock()
            .expect("conversation event subscribers");
        if subscribers.total >= MAX_STREAMS
            || subscribers
                .users
                .get(&user)
                .is_some_and(|u| u.count >= MAX_STREAMS_PER_USER)
        {
            return None;
        }
        let entry = subscribers
            .users
            .entry(user)
            .or_insert_with(|| UserStreams {
                sender: broadcast::channel(CAPACITY).0,
                count: 0,
            });
        let receiver = entry.sender.subscribe();
        entry.count += 1;
        subscribers.total += 1;
        Some(Subscription {
            hub: self.clone(),
            user,
            receiver,
        })
    }
    fn users(&self) -> Vec<Uuid> {
        self.subscribers
            .lock()
            .expect("conversation event subscribers")
            .users
            .keys()
            .copied()
            .collect()
    }
    fn send(&self, users: &[Uuid], notice: Notice) {
        let subscribers = self
            .subscribers
            .lock()
            .expect("conversation event subscribers");
        let wake = Wake::Change(Arc::new(notice));
        for user in users {
            if let Some(entry) = subscribers.users.get(user) {
                let _ = entry.sender.send(wake.clone());
            }
        }
    }
    fn resync(&self, reason: &'static str) {
        let subscribers = self
            .subscribers
            .lock()
            .expect("conversation event subscribers");
        for entry in subscribers.users.values() {
            let _ = entry.sender.send(Wake::Resync(reason));
        }
    }
    fn set_ready(&self, ready: bool) {
        if self.ready.swap(ready, Ordering::SeqCst) != ready {
            self.resync(if ready {
                "listener_connected"
            } else {
                "listener_disconnected"
            });
        }
    }
}
struct Subscription {
    hub: Arc<Hub>,
    user: Uuid,
    receiver: broadcast::Receiver<Wake>,
}
impl Drop for Subscription {
    fn drop(&mut self) {
        let mut subscribers = self
            .hub
            .subscribers
            .lock()
            .expect("conversation event subscribers");
        if let Some(entry) = subscribers.users.get_mut(&self.user) {
            entry.count -= 1;
            if entry.count == 0 {
                subscribers.users.remove(&self.user);
            }
        }
        subscribers.total -= 1;
    }
}

fn hub() -> Arc<Hub> {
    static HUB: OnceLock<Arc<Hub>> = OnceLock::new();
    HUB.get_or_init(|| Arc::new(Hub::new())).clone()
}

/// Called once after migrations. No request-pool connection is retained.
pub fn start() {
    let hub = hub();
    if !hub.started.swap(true, Ordering::SeqCst) {
        tokio::spawn(listener::run(crate::environment::get_database_url(), hub));
    }
}

#[cfg(test)]
mod tests;
