use log::error;
use sea_orm::DatabaseConnection;
use serde::Serialize;
use tokio::join;
use twitch_api::{eventsub::channel::chat::Fragment, types::SubscriptionTier};

use crate::{
    database::entity::{
        events::{EventTrigger, EventTriggerType},
        script_events::ScriptEvent,
        CommandModel, EventModel, ScriptModel,
    },
    twitch::manager::{
        TwitchEventChatMsg, TwitchEventCheerBits, TwitchEventFollow, TwitchEventGiftSub,
        TwitchEventReSub, TwitchEventRedeem, TwitchEventSub, TwitchEventUser,
    },
};

/// Data for matched events to trigger
///
/// TODO: Often small lists, use tinyvec instead?
pub struct EventMatchingData {
    /// List of events to attempt to trigger
    pub events: Vec<EventModel>,

    // TODO: Future
    pub scripts: Vec<ScriptWithContext>,

    /// List of commands to trigger
    pub commands: Vec<CommandWithContext>,

    /// Additional data attached to the event
    pub event_data: EventData,
}

/// Command to trigger with some additional context
pub struct CommandWithContext {
    pub command: CommandModel,

    /// Message with the command/alias removed
    pub message: String,

    /// Args with the first argument command/alias removed
    pub args: Vec<String>,
}

pub struct ScriptWithContext {
    /// The script itself
    pub script: ScriptModel,

    /// Event that was triggered
    pub event: ScriptEvent,
}

#[derive(Default, Debug, Clone, Serialize)]
pub struct EventData {
    /// User who triggered the event
    pub user: Option<TwitchEventUser>,

    /// Additional input data
    #[serde(flatten)]
    pub input_data: EventInputData,
}

/// Additional event-specific input data
#[derive(Debug, Default, Clone, Serialize)]
#[serde(untagged)]
pub enum EventInputData {
    /// No additional input data
    #[default]
    None,

    /// Redeems specific data
    Redeem {
        /// ID of the redeemed award
        reward_id: String,
        /// Name of the reward
        reward_name: String,
        /// Cost of the reward
        cost: i64,
        /// User provided message (For redeems that let you provide a message)
        user_input: String,
    },

    /// Bits specific data
    Bits {
        /// Number of bits given
        bits: i64,
        /// Whether the bits were given anonymously
        anonymous: bool,
        /// User message provided alongside the bits
        message: String,
    },

    /// Subscription specific data
    Subscription {
        /// Tier subscribed at
        tier: SubscriptionTier,
        /// Whether the subscription was gifted
        is_gift: bool,
    },

    /// Gifted Subscription specific data
    GiftedSubscription {
        /// Gifted subscription tier
        tier: SubscriptionTier,
        /// Total gifts user has given (If not anonymous)
        cumulative_total: Option<i64>,
        /// Whether the gifted were given anonymously
        anonymous: bool,
        /// Total subs gifted
        total: i64,
    },

    /// Re-Subscription specific data
    ReSubscription {
        /// The total number of months the user has been subscribed to the channel.
        cumulative_months: i64,
        /// The month duration of the subscription.
        duration_months: i64,
        /// User message attached to the resubscription
        message: String,
        /// The number of consecutive months the user’s current subscription has been active.
        /// This value is null if the user has opted out of sharing this information.
        streak_months: Option<i64>,
        /// Gifted subscription tier
        tier: SubscriptionTier,
    },

    /// Chat message specific data
    Chat {
        /// The chat message itself
        message: String,

        /// Chat message fragments
        fragments: Vec<Fragment>,

        /// Optional amount of bits cheered (If user cheered bits)
        cheer: Option<usize>,
    },
}

pub async fn match_redeem_event(
    db: &DatabaseConnection,
    event: TwitchEventRedeem,
) -> anyhow::Result<EventMatchingData> {
    let (events, scripts) = join!(
        // Loading matching event types
        EventModel::get_by_trigger_type(db, EventTriggerType::Redeem),
        // Load all subscribed scripts
        ScriptModel::get_by_event(db, ScriptEvent::Redeem)
    );

    let events = match events {
        Ok(value) => value,
        Err(err) => {
            error!("failed to load events: {:?}", err);
            Default::default()
        }
    };

    let scripts = match scripts {
        Ok(value) => value,
        Err(err) => {
            error!("failed to load scripts: {:?}", err);
            Default::default()
        }
    };

    // Get requested reward ID
    let event_reward_id = event.reward.id.to_string();

    // Add context to the scripts
    let scripts = scripts
        .into_iter()
        .map(|script| ScriptWithContext {
            script,
            event: ScriptEvent::Redeem,
        })
        .collect();

    // Filter events for the matching reward ID
    let events = events
        .into_iter()
        .filter(|event| {
            matches!(
                &event.trigger,
                EventTrigger::Redeem { reward_id } if event_reward_id.eq(reward_id)
            )
        })
        .collect();

    let event_data = EventData {
        input_data: EventInputData::Redeem {
            reward_id: event_reward_id,
            reward_name: event.reward.title.clone(),
            cost: event.reward.cost,
            user_input: event.user_input,
        },
        user: Some(TwitchEventUser {
            id: event.user_id,
            name: event.user_name,
            display_name: event.user_display_name,
        }),
    };

    Ok(EventMatchingData {
        events,
        scripts,
        commands: Default::default(),
        event_data,
    })
}

pub async fn match_cheer_bits_event(
    db: &DatabaseConnection,
    event: TwitchEventCheerBits,
) -> anyhow::Result<EventMatchingData> {
    let bits = event.bits;

    let (events, scripts) = join!(
        // Loading matching event types
        EventModel::get_by_trigger_type(db, EventTriggerType::Bits),
        // Load all subscribed scripts
        ScriptModel::get_by_event(db, ScriptEvent::CheerBits)
    );

    let events = match events {
        Ok(value) => value,
        Err(err) => {
            error!("failed to load events: {:?}", err);
            Default::default()
        }
    };

    let scripts = match scripts {
        Ok(value) => value,
        Err(err) => {
            error!("failed to load scripts: {:?}", err);
            Default::default()
        }
    };

    // Add context to the scripts
    let scripts = scripts
        .into_iter()
        .map(|script| ScriptWithContext {
            script,
            event: ScriptEvent::CheerBits,
        })
        .collect();

    // Filter events for the matching reward ID
    let events = events
        .into_iter()
        .filter(|event| {
            matches!(&event.trigger, EventTrigger::Bits { min_bits } if bits >= *min_bits as i64)
        })
        .collect();

    // Create user (Bits can be anonymous)
    let user = match (event.user_id, event.user_name, event.user_display_name) {
        (Some(user_id), Some(user_name), Some(user_display_name)) => Some(TwitchEventUser {
            id: user_id,
            name: user_name,
            display_name: user_display_name,
        }),
        _ => None,
    };

    let event_data = EventData {
        input_data: EventInputData::Bits {
            bits: event.bits,
            anonymous: event.anonymous,
            message: event.message,
        },
        user,
    };

    Ok(EventMatchingData {
        events,
        scripts,
        commands: Default::default(),
        event_data,
    })
}

pub async fn match_follow_event(
    db: &DatabaseConnection,
    event: TwitchEventFollow,
) -> anyhow::Result<EventMatchingData> {
    let (events, scripts) = join!(
        // Loading matching event types
        EventModel::get_by_trigger_type(db, EventTriggerType::Follow),
        // Load all subscribed scripts
        ScriptModel::get_by_event(db, ScriptEvent::Follow)
    );

    let events = match events {
        Ok(value) => value,
        Err(err) => {
            error!("failed to load events: {:?}", err);
            Default::default()
        }
    };

    let scripts = match scripts {
        Ok(value) => value,
        Err(err) => {
            error!("failed to load scripts: {:?}", err);
            Default::default()
        }
    };

    // Add context to the scripts
    let scripts = scripts
        .into_iter()
        .map(|script| ScriptWithContext {
            script,
            event: ScriptEvent::Follow,
        })
        .collect();

    let event_data = EventData {
        user: Some(TwitchEventUser {
            id: event.user_id,
            name: event.user_name,
            display_name: event.user_display_name,
        }),
        ..Default::default()
    };

    Ok(EventMatchingData {
        events,
        scripts,
        commands: Default::default(),
        event_data,
    })
}

pub async fn match_subscription_event(
    db: &DatabaseConnection,
    event: TwitchEventSub,
) -> anyhow::Result<EventMatchingData> {
    let (events, scripts) = join!(
        // Loading matching event types
        EventModel::get_by_trigger_type(db, EventTriggerType::Subscription),
        // Load all subscribed scripts
        ScriptModel::get_by_event(db, ScriptEvent::Subscription)
    );

    let events = match events {
        Ok(value) => value,
        Err(err) => {
            error!("failed to load events: {:?}", err);
            Default::default()
        }
    };

    let scripts = match scripts {
        Ok(value) => value,
        Err(err) => {
            error!("failed to load scripts: {:?}", err);
            Default::default()
        }
    };

    // Add context to the scripts
    let scripts = scripts
        .into_iter()
        .map(|script| ScriptWithContext {
            script,
            event: ScriptEvent::Subscription,
        })
        .collect();

    let event_data = EventData {
        input_data: EventInputData::Subscription {
            tier: event.tier,
            is_gift: event.is_gift,
        },
        user: Some(TwitchEventUser {
            id: event.user_id,
            name: event.user_name,
            display_name: event.user_display_name,
        }),
    };

    Ok(EventMatchingData {
        events,
        scripts,
        commands: Default::default(),
        event_data,
    })
}

pub async fn match_gifted_subscription_event(
    db: &DatabaseConnection,
    event: TwitchEventGiftSub,
) -> anyhow::Result<EventMatchingData> {
    let (events, scripts) = join!(
        // Loading matching event types
        EventModel::get_by_trigger_type(db, EventTriggerType::GiftedSubscription),
        // Load all subscribed scripts
        ScriptModel::get_by_event(db, ScriptEvent::GiftSubscription)
    );

    let events = match events {
        Ok(value) => value,
        Err(err) => {
            error!("failed to load events: {:?}", err);
            Default::default()
        }
    };

    let scripts = match scripts {
        Ok(value) => value,
        Err(err) => {
            error!("failed to load scripts: {:?}", err);
            Default::default()
        }
    };

    // Add context to the scripts
    let scripts = scripts
        .into_iter()
        .map(|script| ScriptWithContext {
            script,
            event: ScriptEvent::GiftSubscription,
        })
        .collect();

    // Create user (Bits can be anonymous)
    let user = match (event.user_id, event.user_name, event.user_display_name) {
        (Some(user_id), Some(user_name), Some(user_display_name)) => Some(TwitchEventUser {
            id: user_id,
            name: user_name,
            display_name: user_display_name,
        }),
        _ => None,
    };

    let event_data = EventData {
        input_data: EventInputData::GiftedSubscription {
            tier: event.tier,
            cumulative_total: event.cumulative_total,
            anonymous: event.anonymous,
            total: event.total,
        },
        user,
    };

    Ok(EventMatchingData {
        events,
        scripts,
        commands: Default::default(),
        event_data,
    })
}

pub async fn match_re_subscription_event(
    db: &DatabaseConnection,
    event: TwitchEventReSub,
) -> anyhow::Result<EventMatchingData> {
    let (events, scripts) = join!(
        // Loading matching event types
        EventModel::get_by_trigger_type(db, EventTriggerType::Subscription),
        // Load all subscribed scripts
        ScriptModel::get_by_event(db, ScriptEvent::ReSubscription)
    );

    let events = match events {
        Ok(value) => value,
        Err(err) => {
            error!("failed to load events: {:?}", err);
            Default::default()
        }
    };

    let scripts = match scripts {
        Ok(value) => value,
        Err(err) => {
            error!("failed to load scripts: {:?}", err);
            Default::default()
        }
    };

    // Add context to the scripts
    let scripts = scripts
        .into_iter()
        .map(|script| ScriptWithContext {
            script,
            event: ScriptEvent::ReSubscription,
        })
        .collect();

    let event_data = EventData {
        input_data: EventInputData::ReSubscription {
            cumulative_months: event.cumulative_months,
            duration_months: event.duration_months,
            message: event.message.text,
            streak_months: event.streak_months,
            tier: event.tier,
        },
        user: Some(TwitchEventUser {
            id: event.user_id,
            name: event.user_name,
            display_name: event.user_display_name,
        }),
    };

    Ok(EventMatchingData {
        events,
        scripts,
        commands: Default::default(),
        event_data,
    })
}

pub async fn match_chat_event(
    db: &DatabaseConnection,
    event: TwitchEventChatMsg,
) -> anyhow::Result<EventMatchingData> {
    let message = event.message.text.clone();
    let mut args: Vec<String> = message
        .split_whitespace()
        .map(|value| value.to_string())
        .collect();

    let first_arg = if args.is_empty() {
        None
    } else {
        Some(args.remove(0))
    };

    let (events, commands, scripts) = if let Some(first_arg) = first_arg {
        // Get the command argument from the first argument
        let command_arg = first_arg.trim().to_lowercase();

        let (events, commands, scripts) = join!(
            // Load all command event triggers
            EventModel::get_by_trigger_type(db, EventTriggerType::Command),
            // Load all commands
            CommandModel::get_by_command(db, &command_arg),
            // Load all chat subscribed script
            ScriptModel::get_by_event(db, ScriptEvent::Chat)
        );

        let events = match events {
            Ok(value) => value,
            Err(err) => {
                error!("failed to load events: {:?}", err);
                Default::default()
            }
        };

        let commands = match commands {
            Ok(value) => value,
            Err(err) => {
                error!("failed to load commands: {:?}", err);
                Default::default()
            }
        };

        let scripts = match scripts {
            Ok(value) => value,
            Err(err) => {
                error!("failed to load scripts: {:?}", err);
                Default::default()
            }
        };

        // Add context to the scripts
        let scripts = scripts
            .into_iter()
            .map(|script| ScriptWithContext {
                script,
                event: ScriptEvent::Chat,
            })
            .collect();

        // Filter events for matching command messages
        let events = events
            .into_iter()
            .filter(|event| matches!(&event.trigger, EventTrigger::Command { message } if message.trim().to_lowercase().eq(&command_arg)))
            .collect();

        // Provide additional context to commands
        let commands = commands
            .into_iter()
            .map(|command| {
                // Strip prefix and trim any leading space
                let message = message
                    .strip_prefix(&first_arg)
                    .unwrap_or(&message)
                    .trim_start()
                    .to_string();

                CommandWithContext {
                    command,
                    message,
                    args: args.clone(),
                }
            })
            .collect();

        (events, commands, scripts)
    } else {
        (Default::default(), Default::default(), Default::default())
    };

    let event_data = EventData {
        input_data: EventInputData::Chat {
            message: event.message.text,
            fragments: event.message.fragments,
            cheer: event.cheer.map(|cheer| cheer.bits),
        },
        user: Some(TwitchEventUser {
            id: event.user_id,
            name: event.user_name,
            display_name: event.user_display_name,
        }),
    };

    Ok(EventMatchingData {
        events,
        scripts,
        commands,
        event_data,
    })
}