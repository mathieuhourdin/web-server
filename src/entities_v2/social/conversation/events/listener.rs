use super::{Hub, Notice};
use futures_util::future::poll_fn;
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::mpsc,
    task::JoinHandle,
    time::{interval, sleep, timeout, MissedTickBehavior},
};
use tokio_postgres::{AsyncMessage, Client, NoTls};
use uuid::Uuid;

const CHANNEL: &str = "hupo_conversation_events";

struct Driver(JoinHandle<()>);
impl Drop for Driver {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub(super) async fn run(url: String, hub: Arc<Hub>) {
    let mut backoff = 1;
    loop {
        let started = std::time::Instant::now();
        run_connection(&url, hub.clone()).await;
        hub.set_ready(false);
        // Never include the DSN, notifications or SQL/server details in logs.
        tracing::warn!(target:"conversation_events",retry_in_seconds=backoff,"conversation_events_listener_disconnected");
        sleep(Duration::from_secs(backoff)).await;
        backoff = if started.elapsed() > Duration::from_secs(60) {
            1
        } else {
            (backoff * 2).min(30)
        };
    }
}

async fn run_connection(url: &str, hub: Arc<Hub>) {
    let Ok(mut config) = url.parse::<tokio_postgres::Config>() else {
        return;
    };
    config
        .application_name("hupo_conversation_events")
        .connect_timeout(Duration::from_secs(10))
        .keepalives_idle(Duration::from_secs(30));
    let Ok(Ok((client, mut connection))) =
        timeout(Duration::from_secs(10), config.connect(NoTls)).await
    else {
        return;
    };
    let (sender, mut receiver) = mpsc::channel(256);
    let driver_hub = hub.clone();
    // This task must keep polling the connection while LISTEN/recipient queries
    // await responses. Its bounded queue never blocks the connection driver.
    let _driver = Driver(tokio::spawn(async move {
        while let Some(message) = poll_fn(|cx| connection.poll_message(cx)).await {
            match message {
                Ok(AsyncMessage::Notification(notification))
                    if notification.channel() == CHANNEL =>
                {
                    let Ok(notice) = serde_json::from_str::<Notice>(notification.payload()) else {
                        driver_hub.resync("invalid_notification");
                        continue;
                    };
                    match sender.try_send(notice) {
                        Ok(()) => {}
                        Err(mpsc::error::TrySendError::Full(_)) => {
                            driver_hub.resync("listener_overflow")
                        }
                        Err(mpsc::error::TrySendError::Closed(_)) => break,
                    }
                }
                Err(_) => break,
                _ => {}
            }
        }
        driver_hub.set_ready(false);
    }));
    if !matches!(
        timeout(
            Duration::from_secs(10),
            client.batch_execute("LISTEN hupo_conversation_events")
        )
        .await,
        Ok(Ok(()))
    ) {
        return;
    }
    hub.set_ready(true);
    tracing::info!(target:"conversation_events","conversation_events_listener_connected");
    let mut heartbeat = interval(Duration::from_secs(30));
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            notice=receiver.recv()=> {
                let Some(notice)=notice else {break;};
                let users=hub.users();
                if users.is_empty(){continue;}
                match timeout(Duration::from_secs(10),recipients(&client,&notice,&users)).await {
                    Ok(Ok(users))=>hub.send(&users,notice),
                    _=>break,
                }
            },
            _=heartbeat.tick()=> {
                if !matches!(timeout(Duration::from_secs(10),client.simple_query("SELECT 1")).await,Ok(Ok(_))) {break;}
            },
        }
    }
}

async fn recipients(
    client: &Client,
    notice: &Notice,
    connected: &[Uuid],
) -> Result<Vec<Uuid>, tokio_postgres::Error> {
    match *notice {
        Notice::AccessChanged {
            user_id,
            other_user_id,
        } => {
            return Ok(connected
                .iter()
                .copied()
                .filter(|u| *u == user_id || *u == other_user_id)
                .collect())
        }
        Notice::MemberStateChanged { user_id, .. } => {
            return Ok(connected
                .iter()
                .copied()
                .filter(|u| *u == user_id)
                .collect())
        }
        _ => {}
    }
    let conversation = notice.conversation_id().expect("conversation notice");
    let affected = if let Notice::MembershipChanged { user_id, .. } = notice {
        Some(*user_id)
    } else {
        None
    };
    let position = notice.message_ref().map(|(_, position)| position);
    let rows = client
        .query(
            "SELECT cm.user_id FROM conversation_members cm
        WHERE cm.conversation_id=$1 AND cm.user_id=ANY($2) AND cm.left_at IS NULL
        AND conversation_readable($1,cm.user_id) AND ($4::bigint IS NULL OR cm.history_start_position<=$4)
        UNION SELECT $3::uuid WHERE $3::uuid IS NOT NULL AND $3=ANY($2)",
            &[&conversation, &connected, &affected, &position],
        )
        .await?;
    Ok(rows.into_iter().map(|r| r.get(0)).collect())
}
