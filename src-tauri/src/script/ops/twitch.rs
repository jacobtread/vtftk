use crate::{
    script::events::{
        global_script_event, TwitchGetUserByUsername, TwitchIsMod, TwitchIsVip, TwitchSendChat,
        TwitchSendChatAnnouncement, TwitchSendShoutout,
    },
    twitch::manager::TwitchUser,
};
use anyhow::Context;
use deno_core::*;
use log::debug;
use twitch_api::types::UserId;

/// Operation for sending a chat message from JS
#[op2(async)]
pub async fn op_twitch_send_chat(#[string] message: String) -> anyhow::Result<()> {
    debug!("requested sending twitch chat message: {}", message);

    global_script_event(TwitchSendChat { message })
        .await
        .context("failed to send event")?
}

/// Operation for sending a chat message from JS
#[op2(async)]
pub async fn op_twitch_send_chat_announcement(
    #[string] message: String,
    #[string] color: String,
) -> anyhow::Result<()> {
    debug!(
        "requested sending twitch chat announcement message: {}",
        message
    );

    global_script_event(TwitchSendChatAnnouncement { message, color })
        .await
        .context("failed to send event")?
}

#[op2(async)]
#[serde]
pub async fn op_twitch_get_user_by_username(
    #[string] username: String,
) -> anyhow::Result<Option<TwitchUser>> {
    global_script_event(TwitchGetUserByUsername { username })
        .await
        .context("failed to send event")?
}

#[op2(async)]
pub async fn op_twitch_send_shoutout(#[string] user_id: String) -> anyhow::Result<()> {
    global_script_event(TwitchSendShoutout {
        user_id: UserId::new(user_id),
    })
    .await
    .context("failed to send event")?
}

#[op2(async)]
pub async fn op_twitch_is_mod(#[string] user_id: String) -> anyhow::Result<bool> {
    global_script_event(TwitchIsMod {
        user_id: UserId::new(user_id),
    })
    .await
    .context("failed to send event")?
}

#[op2(async)]
pub async fn op_twitch_is_vip(#[string] user_id: String) -> anyhow::Result<bool> {
    global_script_event(TwitchIsVip {
        user_id: UserId::new(user_id),
    })
    .await
    .context("failed to send event")?
}
