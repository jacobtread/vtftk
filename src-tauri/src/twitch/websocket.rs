use anyhow::Context;
use futures::StreamExt;
use log::warn;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{net::TcpStream, sync::broadcast};
use tokio_tungstenite::{
    tungstenite::{self, protocol::WebSocketConfig},
    MaybeTlsStream, WebSocketStream,
};
use twitch_api::{
    eventsub::{
        self,
        event::websocket::{EventsubWebsocketData, SessionData},
        Event, EventSubscription, EventType, PayloadParseError,
    },
    twitch_oauth2::{self, TwitchToken, UserToken},
    types::{DisplayName, UserId, UserName},
    HelixClient,
};

use tungstenite::{
    error::ProtocolError as WebsocketProtocolError, Error as TWebsocketError,
    Message as WebsocketMessage,
};

use super::manager::{
    TwitchEvent, TwitchEventChatMsg, TwitchEventCheerBits, TwitchEventFollow, TwitchEventGiftSub,
    TwitchEventReSub, TwitchEventRedeem, TwitchEventSub,
};

#[derive(Debug, Error)]
pub enum WebsocketError {
    #[error("token has expired")]
    TokenExpired,

    #[error("twitch access was revoked")]
    Revocation,

    #[error("unexpected message type")]
    UnexpectedMessageType,

    /// Generic error caught
    #[error(transparent)]
    General(#[from] anyhow::Error),

    /// Error occurred in tungstite
    #[error(transparent)]
    Tungstenite(#[from] tungstenite::Error),

    /// Twitch gave back a bad payload
    #[error(transparent)]
    BadPayload(#[from] PayloadParseError),
}

pub struct WebsocketClient {
    /// The session id of the websocket connection
    pub session_id: Option<String>,
    /// The token used to authenticate with the Twitch API
    pub token: UserToken,
    /// The client used to make requests to the Twitch API
    pub client: HelixClient<'static, reqwest::Client>,
    /// The url to use for websocket
    pub connect_url: String,
    /// Sender for twitch events
    pub tx: broadcast::Sender<TwitchEvent>,
}

fn websocket_config() -> WebSocketConfig {
    WebSocketConfig {
        max_message_size: Some(64 << 20), // 64 MiB
        max_frame_size: Some(16 << 20),   // 16 MiB
        accept_unmasked_frames: false,
        ..WebSocketConfig::default()
    }
}

/// Connect to the websocket and return the stream
async fn websocket_connect(
    connect_url: &str,
) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, tungstenite::Error> {
    tokio_tungstenite::connect_async_with_config(connect_url, Some(websocket_config()), false)
        .await
        // We only care about the socket
        .map(|(socket, _)| socket)
}

fn map_message<E: EventSubscription + Clone>(
    message: eventsub::Message<E>,
) -> Result<E::Payload, WebsocketError> {
    match message {
        eventsub::Message::Revocation() => Err(WebsocketError::Revocation),
        eventsub::Message::Notification(msg) => Ok(msg),
        _ => Err(WebsocketError::UnexpectedMessageType),
    }
}

impl WebsocketClient {
    /// Create a new websocket client
    pub fn new(
        client: HelixClient<'static, reqwest::Client>,
        tx: broadcast::Sender<TwitchEvent>,
        token: UserToken,
    ) -> Self {
        Self {
            session_id: None,
            token,
            client,
            connect_url: twitch_api::TWITCH_EVENTSUB_WEBSOCKET_URL.to_string(),
            tx,
        }
    }

    /// Run the websocket subscriber
    pub async fn run(mut self) -> Result<(), WebsocketError> {
        // Establish the stream
        let mut stream = websocket_connect(self.connect_url.as_str())
            .await
            .context("when establishing connection")?;

        while let Some(msg) = stream.next().await {
            let msg = match msg {
                Ok(msg) => msg,
                // Can attempt reconnection from these errors
                Err(TWebsocketError::Protocol(
                    WebsocketProtocolError::ResetWithoutClosingHandshake,
                )) => {
                    warn!("connection lost, reestablishing it");
                    stream = websocket_connect(self.connect_url.as_str())
                        .await
                        .context("when reestablishing connection")?;
                    continue;
                }
                // Other errors can be considered fatal
                Err(err) => return Err(WebsocketError::Tungstenite(err)),
            };

            self.process_message(msg).await?
        }

        Ok(())
    }

    /// Process a message from the websocket
    async fn process_message(&mut self, msg: tungstenite::Message) -> Result<(), WebsocketError> {
        // Only process text messages
        let text = match msg {
            WebsocketMessage::Text(text) => text,
            _ => return Ok(()),
        };

        let event = Event::parse_websocket(&text)?;

        match event {
            // Handle welcome and reconnect
            EventsubWebsocketData::Welcome { payload, .. } => {
                self.initialize_session(payload.session).await?;
            }

            EventsubWebsocketData::Reconnect { payload, .. } => {
                self.initialize_session(payload.session).await?;
            }

            // Handle revocation of permission
            EventsubWebsocketData::Revocation { .. } => return Err(WebsocketError::Revocation),

            // Handle expected messages
            EventsubWebsocketData::Notification { payload, .. } => {
                self.handle_notification(payload).await?
            }

            _ => {}
        }

        Ok(())
    }

    async fn handle_notification(&mut self, event: Event) -> Result<(), WebsocketError> {
        match event {
            // Channel points are redeemed for reward
            Event::ChannelPointsCustomRewardRedemptionAddV1(payload) => {
                let msg: eventsub::channel::ChannelPointsCustomRewardRedemptionAddV1Payload =
                    map_message(payload.message)?;
                _ = self.tx.send(TwitchEvent::Redeem(TwitchEventRedeem {
                    id: msg.id,
                    reward: msg.reward,
                    user_id: msg.user_id,
                    user_name: msg.user_login,
                    user_display_name: msg.user_name,
                }));
            }

            // User sends bits
            Event::ChannelCheerV1(payload) => {
                let msg = map_message(payload.message)?;
                _ = self.tx.send(TwitchEvent::CheerBits(TwitchEventCheerBits {
                    bits: msg.bits,
                    anonymous: msg.is_anonymous,
                    user_id: msg.user_id,
                    user_name: msg.user_login,
                    user_display_name: msg.user_name,
                }));
            }

            // User follows the channel
            Event::ChannelFollowV2(payload) => {
                let msg = map_message(payload.message)?;
                _ = self.tx.send(TwitchEvent::Follow(TwitchEventFollow {
                    user_id: msg.user_id,
                    user_name: msg.user_login,
                    user_display_name: msg.user_name,
                }));
            }

            // User subscribes to channel (does not include resub)
            Event::ChannelSubscribeV1(payload) => {
                let msg = map_message(payload.message)?;
                _ = self.tx.send(TwitchEvent::Sub(TwitchEventSub {
                    is_gift: msg.is_gift,
                    tier: msg.tier,

                    user_id: msg.user_id,
                    user_name: msg.user_login,
                    user_display_name: msg.user_name,
                }));
            }
            // User gifts subscription (1 or more)
            Event::ChannelSubscriptionGiftV1(payload) => {
                let msg = map_message(payload.message)?;
                _ = self.tx.send(TwitchEvent::GiftSub(TwitchEventGiftSub {
                    anonymous: msg.is_anonymous,
                    total: msg.total,
                    cumulative_total: msg.cumulative_total,
                    tier: msg.tier,
                    user_id: msg.user_id,
                    user_name: msg.user_login,
                    user_display_name: msg.user_name,
                }));
            }
            // User sends resubscription message (User sub has resubbed, runs when user sends the resub message to chat)
            Event::ChannelSubscriptionMessageV1(payload) => {
                let msg = map_message(payload.message)?;
                _ = self.tx.send(TwitchEvent::ResubMsg(TwitchEventReSub {
                    cumulative_months: msg.cumulative_months,
                    duration_months: msg.duration_months,
                    message: msg.message,
                    streak_months: msg.streak_months,
                    tier: msg.tier,

                    user_id: msg.user_id,
                    user_name: msg.user_login,
                    user_display_name: msg.user_name,
                }));
            }

            // User sends chat message
            Event::ChannelChatMessageV1(payload) => {
                let msg = map_message(payload.message)?;
                _ = self.tx.send(TwitchEvent::ChatMsg(TwitchEventChatMsg {
                    user_id: msg.chatter_user_id,
                    user_name: msg.chatter_user_login,
                    user_display_name: msg.chatter_user_name,
                    message: msg.message,
                    cheer: msg.cheer,
                }));
            }

            _ => {}
        }

        Ok(())
    }

    /// Initializes a session for the provided session data
    async fn initialize_session(&mut self, data: SessionData<'_>) -> Result<(), WebsocketError> {
        let session_id = data.id.to_string();

        self.session_id = Some(session_id.clone());

        if let Some(url) = data.reconnect_url {
            self.connect_url = url.to_string();
        }

        if self.token.is_elapsed() {
            return Err(WebsocketError::TokenExpired);
        }

        // Subscribe to the desired events
        self.create_subscriptions()
            .await
            .context("creating subscriptions")?;

        Ok(())
    }

    /// Creates subscriptions to the desired events for the current
    /// websocket events session
    async fn create_subscriptions(&self) -> anyhow::Result<()> {
        use eventsub::channel::{
            ChannelChatMessageV1, ChannelCheerV1, ChannelFollowV2,
            ChannelPointsCustomRewardRedemptionAddV1, ChannelSubscribeV1,
            ChannelSubscriptionGiftV1, ChannelSubscriptionMessageV1,
        };

        let session_id = self.session_id.as_deref().context("no active session")?;

        let token = &self.token;
        let user_id = token.user_id.clone();

        let transport = eventsub::Transport::websocket(session_id);

        // Subscribe to reward redemptions
        self.client
            .create_eventsub_subscription(
                ChannelPointsCustomRewardRedemptionAddV1::broadcaster_user_id(user_id.clone()),
                transport.clone(),
                token,
            )
            .await
            .context("subscribe redeems")?;

        // Subscribe to bits cheering
        self.client
            .create_eventsub_subscription(
                ChannelCheerV1::broadcaster_user_id(user_id.clone()),
                transport.clone(),
                token,
            )
            .await
            .context("subscribe cheers")?;

        // Subscribe to channel follows
        self.client
            .create_eventsub_subscription(
                ChannelFollowV2::new(user_id.clone(), user_id.clone()),
                transport.clone(),
                token,
            )
            .await
            .context("subscribe follows")?;

        // Subscribe to channel subscriptions
        self.client
            .create_eventsub_subscription(
                ChannelSubscribeV1::broadcaster_user_id(user_id.clone()),
                transport.clone(),
                token,
            )
            .await
            .context("subscribe subs")?;

        // Subscribe to channel gifted subscriptions
        self.client
            .create_eventsub_subscription(
                ChannelSubscriptionGiftV1::broadcaster_user_id(user_id.clone()),
                transport.clone(),
                token,
            )
            .await
            .context("subscribe gifted subs")?;

        // Subscribe to channel resub message
        self.client
            .create_eventsub_subscription(
                ChannelSubscriptionMessageV1::broadcaster_user_id(user_id.clone()),
                transport.clone(),
                token,
            )
            .await
            .context("subscribe resub message")?;

        // Subscribe to channel chat message
        self.client
            .create_eventsub_subscription(
                ChannelChatMessageV1::new(user_id.clone(), user_id.clone()),
                transport.clone(),
                token,
            )
            .await
            .context("subscribe message")?;

        Ok(())
    }
}

// #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
// pub struct ChannelSubscriptionModeratorAdd {
//     /// The broadcaster user ID.
//     pub broadcaster_user_id: UserId,
// }

// impl EventSubscription for ChannelSubscriptionModeratorAdd {
//     type Payload = ChannelSubscriptionModeratorAddPayload;

//     const EVENT_TYPE: EventType = EventType::ChannelSubscription;
//     const SCOPE: twitch_oauth2::Validator =
//         twitch_oauth2::validator![twitch_oauth2::Scope::ModerationRead];
//     const VERSION: &'static str = "1";
// }

// #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
// pub struct ChannelSubscriptionModeratorAddPayload {
//     /// The broadcaster user ID.
//     pub broadcaster_user_id: UserId,
//     /// The broadcaster login.
//     pub broadcaster_user_login: DisplayName,
//     /// The broadcaster display name.
//     pub broadcaster_user_name: UserName,
// }
