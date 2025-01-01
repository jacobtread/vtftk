use std::{
    collections::BinaryHeap, future::Future, pin::Pin, sync::Arc, task::Poll, time::Duration,
};

use anyhow::Context;
use chrono::Local;
use futures::future::BoxFuture;
use log::error;
use sea_orm::DatabaseConnection;
use tokio::{
    sync::{broadcast, mpsc},
    time::{sleep_until, Instant},
};

use crate::{
    database::entity::{
        events::{EventTrigger, EventTriggerType},
        EventModel,
    },
    script::runtime::ScriptExecutorHandle,
    twitch::manager::TwitchManager,
};

use super::{
    matching::{EventData, EventInputData},
    processing::execute_event,
    EventMessage,
};

pub struct ScheduledEvent {
    pub event: EventModel,
    /// Next instance the
    pub next_run: Instant,
}

impl Eq for ScheduledEvent {}

impl PartialEq for ScheduledEvent {
    fn eq(&self, other: &Self) -> bool {
        self.event.id.eq(&other.event.id)
    }
}

impl PartialOrd for ScheduledEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse comparison order for binary heap to sort
        // closest ones to the top
        other.next_run.cmp(&self.next_run)
    }
}

#[derive(Clone)]
pub struct SchedulerHandle(mpsc::Sender<Vec<EventModel>>);

impl SchedulerHandle {
    pub async fn update_events(&self, events: Vec<EventModel>) -> anyhow::Result<()> {
        self.0.send(events).await.context("failed to send event")
    }
}

pub fn create_scheduler(
    db: DatabaseConnection,
    twitch_manager: Arc<TwitchManager>,
    script_handle: ScriptExecutorHandle,
    event_sender: broadcast::Sender<EventMessage>,
) -> SchedulerHandle {
    let (tx, rx) = mpsc::channel(5);

    // Load the initial events data
    tauri::async_runtime::spawn({
        let db = db.clone();
        let tx = tx.clone();

        async move {
            let events = match EventModel::get_by_trigger_type(&db, EventTriggerType::Timer).await {
                Ok(events) => events,
                _ => return,
            };

            _ = tx.send(events).await;
        }
    });

    tauri::async_runtime::spawn(SchedulerEventLoop {
        rx,
        events: BinaryHeap::new(),
        current_sleep: None,
        db,
        twitch_manager,
        script_handle,
        event_sender,
    });

    SchedulerHandle(tx)
}

struct SchedulerEventLoop {
    /// Receiver for the latest events list
    rx: mpsc::Receiver<Vec<EventModel>>,

    /// Heap of scheduled events, ordered by the event which is
    /// due to come first
    events: BinaryHeap<ScheduledEvent>,

    /// Current sleep future
    current_sleep: Option<BoxFuture<'static, ()>>,

    db: DatabaseConnection,
    twitch_manager: Arc<TwitchManager>,
    script_handle: ScriptExecutorHandle,
    event_sender: broadcast::Sender<EventMessage>,
}

impl SchedulerEventLoop {
    fn execute_scheduled_event(&self, event: EventModel) {
        let db = self.db.clone();
        let twitch_manager = self.twitch_manager.clone();
        let script_handle = self.script_handle.clone();
        let event_sender = self.event_sender.clone();

        tauri::async_runtime::spawn(async move {
            let db = db;
            let twitch_manager = twitch_manager;
            let script_handle = script_handle;
            let event_sender = event_sender;

            if let Err(err) = execute_event(
                &db,
                &twitch_manager,
                &script_handle,
                &event_sender,
                event,
                EventData {
                    user: None,
                    input_data: EventInputData::None,
                },
            )
            .await
            {
                error!("error while executing event outcome (in timer): {err:?}");
            }
        });
    }

    fn poll_inner(&mut self, cx: &mut std::task::Context<'_>) -> Poll<()> {
        // Accept messages to update the events list
        while let Poll::Ready(Some(events)) = self.rx.poll_recv(cx) {
            // Create the scheduled events
            self.events = events
                .into_iter()
                .filter_map(create_scheduled_event)
                .collect();

            // Clear sleep state
            self.current_sleep = None;
        }

        if let Some(current_sleep) = self.current_sleep.as_mut() {
            // Poll current sleep
            if Pin::new(current_sleep).poll(cx).is_pending() {
                return Poll::Pending;
            }

            // Clear current sleep
            self.current_sleep = None;

            // Value should always be present when we have awaited a sleep state
            let event = match self.events.pop() {
                Some(value) => value,
                None => return Poll::Pending,
            };

            // Trigger the event
            self.execute_scheduled_event(event.event.clone());

            if let Some(event) = create_scheduled_event(event.event) {
                self.events.push(event);
            }

            // Emit event
            return Poll::Ready(());
        }

        // Peek the top event
        let next_event = match self.events.peek() {
            Some(value) => value,
            None => return Poll::Pending,
        };

        // Store and poll new sleep state
        let sleep = sleep_until(next_event.next_run);
        let sleep = self.current_sleep.insert(Box::pin(sleep));

        Pin::new(sleep).poll(cx)
    }
}

impl Future for SchedulerEventLoop {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();

        // Poll inner until its no longer ready
        while this.poll_inner(cx).is_ready() {}

        Poll::Pending
    }
}

fn create_scheduled_event(event: EventModel) -> Option<ScheduledEvent> {
    let interval = match &event.trigger {
        EventTrigger::Timer { interval } => *interval,
        _ => return None,
    };

    let next_run = get_next_interval_instant(interval);
    Some(ScheduledEvent { event, next_run })
}

/// Gets the next instant for a fixed interval
fn get_next_interval_instant(interval: u64) -> Instant {
    let now = Local::now();
    let seconds_since_epoch = now.timestamp() as u64;
    let next = (seconds_since_epoch / interval + 1) * interval;
    Instant::now() + Duration::from_secs(next - seconds_since_epoch)
}