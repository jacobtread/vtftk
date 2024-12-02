use std::sync::Arc;

use anyhow::Context;
use serde::Serialize;
use tokio::sync::{mpsc, oneshot, Mutex};
use twitch_api::{
    helix::chat::{SendChatMessageBody, SendChatMessageRequest, SendChatMessageResponse},
    types::{DisplayName, UserId, UserName},
};

use crate::{state::app_data::AppDataStore, twitch::manager::TwitchManager};

use super::kv::KVStore;

/// Events coming from the JS runtime to be executed by a locally
/// managed handler with state accessible only from the main app
#[allow(clippy::enum_variant_names)]
pub enum JsEventMessage {
    /// Trigger sending a twitch chat message
    TwitchSendChat {
        /// The message to send
        message: String,
        /// Channel to respond through with the outcome to sending the message
        return_tx: oneshot::Sender<anyhow::Result<()>>,
    },

    /// Trigger checking if a user is a twitch moderator for
    /// the channel
    TwitchIsMod {
        /// The ID of the user to check
        user_id: UserId,
        /// Channel to respond through with the outcome
        return_tx: oneshot::Sender<anyhow::Result<bool>>,
    },

    /// Trigger checking if a user is a twitch vip for
    /// the channel
    TwitchIsVip {
        /// The ID of the user to check
        user_id: UserId,
        /// Channel to respond through with the outcome
        return_tx: oneshot::Sender<anyhow::Result<bool>>,
    },

    KvSet {
        key: String,
        value: String,
        return_tx: oneshot::Sender<anyhow::Result<()>>,
    },

    KvGet {
        key: String,
        return_tx: oneshot::Sender<anyhow::Result<Option<String>>>,
    },

    KvRemove {
        key: String,
        return_tx: oneshot::Sender<anyhow::Result<()>>,
    },
}

/// Event coming from outside the JS runtime to trigger executing
/// code within the runtime event listeners
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ScriptExecuteEvent {
    Chat {
        user_id: UserId,
        user_name: UserName,
        user_display_name: DisplayName,
        message: String,
    },
}

/// Currently active sender for [JsEventMessage]s
pub static SCRIPT_EVENT_PRODUCER: Mutex<Option<mpsc::Sender<JsEventMessage>>> =
    Mutex::const_new(None);

/// Initializes global script handling using the provided dependencies
pub fn init_script_event_handling(
    app_data_store: AppDataStore,
    kv_store: KVStore,
    twitch_manager: Arc<TwitchManager>,
) {
    let (tx, rx) = mpsc::channel(10);

    // Can block here, initialization will never have any other writers so won't be blocked
    *SCRIPT_EVENT_PRODUCER.blocking_lock() = Some(tx);

    // Spawn background task to process events
    tauri::async_runtime::spawn(handle_script_events(
        app_data_store,
        twitch_manager,
        kv_store,
        rx,
    ));
}

/// Asynchronous task handling for receiving events then dispatching tasks
/// to executed the event action
pub async fn handle_script_events(
    _app_data_store: AppDataStore,
    twitch_manager: Arc<TwitchManager>,
    kv: KVStore,
    mut rx: mpsc::Receiver<JsEventMessage>,
) {
    while let Some(event) = rx.recv().await {
        match event {
            JsEventMessage::TwitchSendChat { message, return_tx } => {
                let twitch_manager = twitch_manager.clone();
                tokio::spawn(async move {
                    let result = handle_twitch_send_chat_event(twitch_manager, message).await;
                    _ = return_tx.send(result);
                });
            }

            JsEventMessage::TwitchIsMod { user_id, return_tx } => {
                let twitch_manager = twitch_manager.clone();
                tokio::spawn(async move {
                    let result = handle_twitch_is_mod_event(twitch_manager, user_id).await;
                    _ = return_tx.send(result);
                });
            }

            JsEventMessage::TwitchIsVip { user_id, return_tx } => {
                let twitch_manager = twitch_manager.clone();
                tokio::spawn(async move {
                    let result = handle_twitch_is_vip_event(twitch_manager, user_id).await;
                    _ = return_tx.send(result);
                });
            }
            JsEventMessage::KvSet {
                key,
                value,
                return_tx,
            } => {
                let kv = kv.clone();
                tokio::spawn(async move {
                    let result = handle_kv_set_event(kv, key, value).await;
                    _ = return_tx.send(result);
                });
            }
            JsEventMessage::KvGet { key, return_tx } => {
                let kv = kv.clone();
                tokio::spawn(async move {
                    let result = handle_kv_get_event(kv, key).await;
                    _ = return_tx.send(result);
                });
            }
            JsEventMessage::KvRemove { key, return_tx } => {
                let kv = kv.clone();
                tokio::spawn(async move {
                    let result = handle_kv_remove_event(kv, key).await;
                    _ = return_tx.send(result);
                });
            }
        }
    }
}

/// Handles a twitch chat event, sends a twitch chat message
async fn handle_twitch_send_chat_event(
    twitch_manager: Arc<TwitchManager>,
    message: String,
) -> anyhow::Result<()> {
    let token = twitch_manager
        .get_user_token()
        .await
        .context("not authenticated")?;
    let user_id = token.user_id.clone();
    let request = SendChatMessageRequest::new();
    let body = SendChatMessageBody::new(user_id.clone(), user_id, message);
    let _response: SendChatMessageResponse = twitch_manager
        .helix_client
        .req_post(request, body, &token)
        .await?
        .data;

    Ok(())
}

async fn handle_twitch_is_mod_event(
    twitch_manager: Arc<TwitchManager>,
    user_id: UserId,
) -> anyhow::Result<bool> {
    let mods = twitch_manager.get_moderator_list().await?;
    Ok(mods.iter().any(|vip| vip.user_id == user_id))
}

async fn handle_twitch_is_vip_event(
    twitch_manager: Arc<TwitchManager>,
    user_id: UserId,
) -> anyhow::Result<bool> {
    let vips = twitch_manager.get_vip_list().await?;
    Ok(vips.iter().any(|vip| vip.user_id == user_id))
}

async fn handle_kv_set_event(kv_store: KVStore, key: String, value: String) -> anyhow::Result<()> {
    kv_store.set(&key, value).await?;
    Ok(())
}

async fn handle_kv_remove_event(kv_store: KVStore, key: String) -> anyhow::Result<()> {
    kv_store.remove(&key).await?;
    Ok(())
}

async fn handle_kv_get_event(kv_store: KVStore, key: String) -> anyhow::Result<Option<String>> {
    let value = kv_store.get(&key).await;
    Ok(value)
}
