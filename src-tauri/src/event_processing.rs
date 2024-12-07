use crate::database::entity::commands::CommandOutcome;
use crate::database::entity::events::{BitsAmount, EventOutcome, EventTrigger, ThrowableData};
use crate::database::entity::shared::{DbResult, MinimumRequireRole};
use crate::database::entity::{CommandModel, EventModel, ItemModel, ScriptModel, SoundModel};
use crate::events::EventMessage;
use crate::script::runtime::{
    CommandContext, CommandContextUser, ScriptExecuteEvent, ScriptExecutorHandle,
};
use crate::state::app_data::{ItemWithImpactSoundIds, ThrowableConfig};
use crate::twitch::manager::{
    TwitchEvent, TwitchEventChatMsg, TwitchEventCheerBits, TwitchEventFollow, TwitchEventGiftSub,
    TwitchEventReSub, TwitchEventRedeem, TwitchEventSub, TwitchEventUser, TwitchManager,
};
use anyhow::Context;
use futures::future::BoxFuture;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use log::{debug, error};
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use std::{sync::Arc, time::Duration};
use tokio::sync::{broadcast, RwLock};
use tokio::time::Instant;
use tokio::try_join;
use twitch_api::types::UserId;
use uuid::Uuid;

#[derive(Default)]
pub struct EventsState {
    // Last execution time per event
    pub event_last_execution: HashMap<Uuid, Instant>,
}

#[derive(Default, Clone)]
pub struct EventsStateShared {
    inner: Arc<RwLock<EventsState>>,
}

impl EventsStateShared {
    pub async fn is_cooldown_elapsed(&self, uuid: &Uuid, cooldown: Duration) -> bool {
        let now = Instant::now();

        let inner = &*self.inner.read().await;
        if let Some(last_instant) = inner.event_last_execution.get(uuid) {
            let elapsed = now.duration_since(*last_instant);
            elapsed > cooldown
        } else {
            true
        }
    }

    pub async fn set_last_executed(&self, uuid: &Uuid) {
        let now = Instant::now();
        let inner = &mut *self.inner.write().await;
        inner.event_last_execution.insert(*uuid, now);
    }
}

pub async fn get_scripts_by_event(db: &DatabaseConnection, name: &str) -> Vec<ScriptModel> {
    let scripts = match ScriptModel::all(db).await {
        Ok(value) => value,
        Err(err) => {
            error!("failed to load scripts: {:?}", err);
            return Vec::new();
        }
    };

    scripts
        .into_iter()
        .filter(|script| script.enabled && script.events.0.iter().any(|event| name.eq(event)))
        .collect()
}

pub fn execute_scripts(
    script_handle: ScriptExecutorHandle,
    scripts: Vec<ScriptModel>,
    event: ScriptExecuteEvent,
) {
    // Spawn task to poll the execute futures
    tokio::spawn(async move {
        // Create futures to execute for each config
        let mut futures = scripts
            .into_iter()
            .map(|script_config| -> BoxFuture<'_, anyhow::Result<()>> {
                Box::pin(script_handle.execute(script_config.script, event.clone()))
            })
            .collect::<FuturesUnordered<BoxFuture<'_, anyhow::Result<()>>>>();

        while let Some(value) = futures.next().await {
            if let Err(err) = value {
                error!("failed to execute script: {:?}", err);
            }
        }
    });
}

pub fn execute_commands(
    script_handle: ScriptExecutorHandle,
    twitch_manager: Arc<TwitchManager>,
    events_state: EventsStateShared,
    commands: Vec<(CommandModel, CommandContext)>,
) {
    // Spawn task to poll the execute futures
    tokio::spawn(async move {
        // Create futures to execute for each config
        let mut futures = commands
            .into_iter()
            .map(|(config, ctx)| -> BoxFuture<'_, anyhow::Result<()>> {
                Box::pin(execute_command(
                    &script_handle,
                    &twitch_manager,
                    &events_state,
                    config,
                    ctx,
                ))
            })
            .collect::<FuturesUnordered<BoxFuture<'_, anyhow::Result<()>>>>();

        while let Some(value) = futures.next().await {
            if let Err(err) = value {
                error!("failed to execute script: {:?}", err);
            }
        }
    });
}

pub async fn execute_command(
    script_handle: &ScriptExecutorHandle,
    twitch_manager: &Arc<TwitchManager>,
    events_state: &EventsStateShared,
    config: CommandModel,
    ctx: CommandContext,
) -> anyhow::Result<()> {
    // Ensure required role is present
    if !assert_required_role(
        twitch_manager,
        Some(ctx.user.id.clone()),
        &config.require_role,
    )
    .await
    {
        debug!("skipping event: missing required role");
        return Ok(());
    }

    // Ensure cooldown is not active
    if !events_state
        .is_cooldown_elapsed(&config.id, Duration::from_millis(config.cooldown as u64))
        .await
    {
        debug!("skipping command: cooldown");
        return Ok(());
    }

    match config.outcome {
        CommandOutcome::Template { message: _message } => {
            // TODO: Not implemented yet
        }
        CommandOutcome::Script { script } => {
            script_handle.execute_command(script, ctx).await?;
        }
    }

    // Mark last execution for cooldown
    events_state.set_last_executed(&config.id).await;

    Ok(())
}

pub fn get_target_commands(
    commands: &[CommandModel],
    event: &TwitchEventChatMsg,
) -> Vec<(CommandModel, CommandContext)> {
    commands
        .iter()
        .filter(|command| command.enabled)
        .filter_map(|command| {
            let message = event.message.text.clone();
            let mut args: Vec<String> = message
                .split_whitespace()
                .map(|value| value.to_string())
                .collect();

            // Must have at least one arg to be a command
            if args.is_empty() {
                return None;
            }

            let first_arg = args.remove(0);

            // Ensure the command matches the first arg
            if !first_arg.eq_ignore_ascii_case(&command.command)
                && !command
                    .aliases
                    .0
                    .iter()
                    .any(|alias| first_arg.eq_ignore_ascii_case(alias))
            {
                return None;
            }

            // Strip prefix and trim any leading space
            let without_prefix = message
                .strip_prefix(&first_arg)
                .unwrap_or(&message)
                .trim_start()
                .to_string();

            let user = CommandContextUser {
                id: event.user_id.clone(),
                name: event.user_name.clone(),
                display_name: event.user_display_name.clone(),
            };

            Some((
                command.clone(),
                CommandContext {
                    full_message: event.message.text.clone(),
                    message: without_prefix,
                    user,
                    args,
                },
            ))
        })
        .collect()
}

pub async fn handle_twitch_events(
    db: DatabaseConnection,
    twitch_manager: Arc<TwitchManager>,
    mut twitch_event_rx: broadcast::Receiver<TwitchEvent>,
    event_sender: broadcast::Sender<EventMessage>,
    script_handle: ScriptExecutorHandle,
) {
    let events_state = EventsStateShared::default();

    while let Ok(event) = twitch_event_rx.recv().await {
        debug!("twitch event received: {:?}", event);

        let events = match EventModel::all(&db).await {
            Ok(value) => value,
            Err(err) => {
                error!("failed to load events: {:?}", err);
                continue;
            }
        };

        let mut data = match event {
            // Handle trigger events
            TwitchEvent::Redeem(event) => get_redeem_event_data(events, event),
            TwitchEvent::CheerBits(event) => get_cheer_bits_event_data(events, event),
            TwitchEvent::Follow(event) => get_follow_event_data(events, event),
            TwitchEvent::Sub(event) => get_sub_event_data(events, event),
            TwitchEvent::GiftSub(event) => get_gift_sub_event_data(events, event),
            TwitchEvent::ResubMsg(event) => get_resub_event_data(events, event),
            TwitchEvent::ChatMsg(event) => {
                // Execute commands
                // TODO: Query commands from the database using the parsed command message
                match CommandModel::all(&db).await {
                    Ok(commands) => {
                        let commands = get_target_commands(&commands, &event);
                        execute_commands(
                            script_handle.clone(),
                            twitch_manager.clone(),
                            events_state.clone(),
                            commands,
                        );
                    }
                    Err(err) => {
                        error!("failed to load commands: {:?}", err);
                    }
                }

                let scripts = get_scripts_by_event(&db, "chat").await;

                if !scripts.is_empty() {
                    execute_scripts(
                        script_handle.clone(),
                        scripts,
                        ScriptExecuteEvent::Chat {
                            message: event.message.text.clone(),
                            user_id: event.user_id.clone(),
                            user_name: event.user_name.clone(),
                            user_display_name: event.user_display_name.clone(),
                        },
                    );
                }

                get_chat_event_data(events, event)
            }

            // Handle change events from websockets
            TwitchEvent::ModeratorsChanged => {
                let twitch_manager = twitch_manager.clone();
                tokio::spawn(async move {
                    debug!("reloading mods list");
                    _ = twitch_manager.load_moderator_list().await;
                });

                continue;
            }
            TwitchEvent::VipsChanged => {
                let twitch_manager = twitch_manager.clone();
                tokio::spawn(async move {
                    debug!("reloading vips list");
                    _ = twitch_manager.load_vip_list().await;
                });

                continue;
            }
            TwitchEvent::RewardsChanged => {
                let twitch_manager = twitch_manager.clone();
                tokio::spawn(async move {
                    debug!("reloading rewards list");
                    _ = twitch_manager.load_rewards_list().await;
                });

                continue;
            }
            TwitchEvent::Reset => {
                let twitch_manager = twitch_manager.clone();
                tokio::spawn(async move {
                    debug!("resetting twitch manager");
                    twitch_manager.reset().await;
                });

                continue;
            }
        };

        // Remove any events that aren't enabled
        data.event_configs
            .retain(|event_config| event_config.enabled);

        // Skip expensive cloning when no events
        if data.event_configs.is_empty() {
            continue;
        }

        // Create futures to execute for each config
        let mut futures = data
            .event_configs
            .into_iter()
            .map(|event_config| -> BoxFuture<'static, ()> {
                Box::pin(process_event_config(
                    db.clone(),
                    twitch_manager.clone(),
                    event_sender.clone(),
                    events_state.clone(),
                    data.event_data.clone(),
                    event_config,
                ))
            })
            .collect::<FuturesUnordered<BoxFuture<'static, ()>>>();

        // Spawn task to poll the execute futures
        tokio::spawn(async move { while (futures.next().await).is_some() {} });
    }
}

async fn process_event_config(
    db: DatabaseConnection,
    twitch_manager: Arc<TwitchManager>,
    event_sender: broadcast::Sender<EventMessage>,
    events_state: EventsStateShared,
    event_data: EventData,
    event_config: EventModel,
) {
    // Ensure required role is present
    if !assert_required_role(
        &twitch_manager,
        event_data.user.as_ref().map(|value| value.user_id.clone()),
        &event_config.require_role,
    )
    .await
    {
        debug!("skipping event: missing required role");
        return;
    }

    let id = event_config.id;

    // Ensure cooldown is not active
    if !events_state
        .is_cooldown_elapsed(&id, Duration::from_millis(event_config.cooldown as u64))
        .await
    {
        debug!("skipping event: cooldown");
        return;
    }

    // Wait for the delay to complete
    let delay = Duration::from_millis(event_config.outcome_delay as u64);
    tokio::time::sleep(delay).await;

    match get_outcome_event_message(&db, event_config, event_data).await {
        Ok(msg) => {
            _ = event_sender.send(msg);
            events_state.set_last_executed(&id).await;
        }
        Err(err) => {
            error!("failed to produce event outcome:\n{err:?}");
        }
    };
}

#[derive(Debug)]
pub struct EventHandleData {
    event_configs: Vec<EventModel>,
    event_data: EventData,
}

fn get_redeem_event_data(events: Vec<EventModel>, event: TwitchEventRedeem) -> EventHandleData {
    let event_reward_id = event.reward.id.to_string();
    let event_configs: Vec<EventModel> = events
        .into_iter()
        .filter(|event_config| {
            matches!(&event_config.trigger, EventTrigger::Redeem { reward_id } if event_reward_id.eq(reward_id))
        })
        .collect();

    let event_data = EventData {
        input: None,
        user: Some(TwitchEventUser {
            user_id: event.user_id,
            user_name: event.user_name,
            user_display_name: event.user_display_name,
        }),
    };

    EventHandleData {
        event_configs,
        event_data,
    }
}

fn get_cheer_bits_event_data(
    events: Vec<EventModel>,
    event: TwitchEventCheerBits,
) -> EventHandleData {
    let event_configs: Vec<EventModel> = events
        .into_iter()
        .filter(|event_config| {
            matches!(&event_config.trigger, EventTrigger::Bits { min_bits } if event.bits >= *min_bits as i64)
        })
        .collect();

    let user = match (event.user_id, event.user_name, event.user_display_name) {
        (Some(user_id), Some(user_name), Some(user_display_name)) => Some(TwitchEventUser {
            user_id,
            user_name,
            user_display_name,
        }),
        _ => None,
    };

    let event_data = EventData {
        input: Some(event.bits as u32),
        user,
    };

    EventHandleData {
        event_configs,
        event_data,
    }
}

fn get_follow_event_data(events: Vec<EventModel>, event: TwitchEventFollow) -> EventHandleData {
    let event_configs: Vec<EventModel> = events
        .into_iter()
        .filter(|event_config| matches!(&event_config.trigger, EventTrigger::Follow))
        .collect();

    let event_data = EventData {
        input: None,
        user: Some(TwitchEventUser {
            user_id: event.user_id,
            user_name: event.user_name,
            user_display_name: event.user_display_name,
        }),
    };

    EventHandleData {
        event_configs,
        event_data,
    }
}

fn get_sub_event_data(events: Vec<EventModel>, event: TwitchEventSub) -> EventHandleData {
    let event_configs: Vec<EventModel> = events
        .into_iter()
        .filter(|event_config| matches!(&event_config.trigger, EventTrigger::Subscription))
        .collect();

    let event_data = EventData {
        input: None,
        user: Some(TwitchEventUser {
            user_id: event.user_id,
            user_name: event.user_name,
            user_display_name: event.user_display_name,
        }),
    };

    EventHandleData {
        event_configs,
        event_data,
    }
}

fn get_gift_sub_event_data(events: Vec<EventModel>, event: TwitchEventGiftSub) -> EventHandleData {
    let event_configs: Vec<EventModel> = events
        .into_iter()
        .filter(|event_config| matches!(&event_config.trigger, EventTrigger::GiftedSubscription))
        .collect();

    let user = match (event.user_id, event.user_name, event.user_display_name) {
        (Some(user_id), Some(user_name), Some(user_display_name)) => Some(TwitchEventUser {
            user_id,
            user_name,
            user_display_name,
        }),
        _ => None,
    };

    let event_data = EventData { input: None, user };

    EventHandleData {
        event_configs,
        event_data,
    }
}

fn get_resub_event_data(events: Vec<EventModel>, event: TwitchEventReSub) -> EventHandleData {
    let event_configs: Vec<EventModel> = events
        .into_iter()
        .filter(|event_config| matches!(&event_config.trigger, EventTrigger::Subscription))
        .collect();

    let event_data = EventData {
        input: None,
        user: Some(TwitchEventUser {
            user_id: event.user_id,
            user_name: event.user_name,
            user_display_name: event.user_display_name,
        }),
    };

    EventHandleData {
        event_configs,
        event_data,
    }
}

fn get_chat_event_data(events: Vec<EventModel>, event: TwitchEventChatMsg) -> EventHandleData {
    let event_configs: Vec<EventModel> = events
        .into_iter()
        .filter(|event_config| match &event_config.trigger {
            EventTrigger::Command { message } => {
                let left = message.trim().to_lowercase();
                let right = event
                    .message
                    .fragments
                    .first()
                    .map(|frag| frag.text())
                    .unwrap_or_default()
                    .trim()
                    .to_lowercase();

                left == right
            }
            _ => false,
        })
        .collect();

    let event_data = EventData {
        input: None,
        user: Some(TwitchEventUser {
            user_id: event.user_id,
            user_name: event.user_name,
            user_display_name: event.user_display_name,
        }),
    };

    EventHandleData {
        event_configs,
        event_data,
    }
}

#[derive(Debug, Clone)]
pub struct EventData {
    /// Represents the data provided by the trigger, i.e amount of bits
    /// total number of subs, number of raiders etc
    input: Option<u32>,

    /// User who triggered the event
    user: Option<TwitchEventUser>,
}

async fn assert_required_role(
    twitch_manager: &TwitchManager,
    user_id: Option<UserId>,
    required_role: &MinimumRequireRole,
) -> bool {
    match required_role {
        MinimumRequireRole::None => true,
        MinimumRequireRole::Vip => {
            let user = match user_id {
                Some(user) => user,
                None => return false,
            };

            // User is the broadcaster
            if twitch_manager
                .get_user_token()
                .await
                .is_some_and(|value| value.user_id == user)
            {
                return true;
            }

            let (vips, mods) = match try_join!(
                twitch_manager.get_vip_list(),
                twitch_manager.get_moderator_list()
            ) {
                Ok(value) => value,
                Err(_) => return false,
            };

            vips.iter().any(|vip| vip.user_id == user)
                || mods.iter().any(|mods| mods.user_id == user)
        }
        MinimumRequireRole::Mod => {
            let user = match user_id {
                Some(user) => user,
                None => return false,
            };

            // User is the broadcaster
            if twitch_manager
                .get_user_token()
                .await
                .is_some_and(|value| value.user_id == user)
            {
                return true;
            }

            let mods = match twitch_manager.get_moderator_list().await {
                Ok(value) => value,
                Err(_) => {
                    return false;
                }
            };

            mods.iter().any(|mods| mods.user_id == user)
        }
    }
}

async fn get_outcome_event_message(
    db: &DatabaseConnection,
    event_config: EventModel,
    event_data: EventData,
) -> anyhow::Result<EventMessage> {
    match event_config.outcome {
        EventOutcome::ThrowBits(data) => {
            let input = event_data.input.context("throw bits missing input")?;
            let sets = [data._1, data._100, data._1000, data._5000, data._10000];
            let mut bit_index: usize = match input {
                1..=99 => 0,
                100..=999 => 1,
                1000..=4999 => 2,
                5000..=9999 => 3,
                _ => 4,
            };

            let mut bit_icon: Option<Uuid> = None;

            // Go through the bit icons till we find one
            while bit_icon.is_none() {
                bit_icon = sets.get(bit_index).and_then(|value| *value);

                // Move to index before
                match bit_index.checked_sub(1) {
                    Some(value) => {
                        bit_index = value;
                    }
                    None => break,
                }
            }

            let bit_icon = bit_icon.context("no bit icon available")?;

            let item = ItemModel::get_by_id(db, bit_icon)
                .await?
                .context("bit icon item missing")?;

            let throwable_config = create_throwable_config(db, vec![item]).await?;

            let amount = match data.amount {
                BitsAmount::Dynamic { max_amount } => input.min(max_amount),
                BitsAmount::Fixed { amount } => amount,
            };

            Ok(EventMessage::ThrowItem {
                config: throwable_config,
                amount,
            })
        }
        EventOutcome::Throwable(data) => match data.data {
            ThrowableData::Throw {
                throwable_ids,
                amount,
            } => {
                let items = ItemModel::get_by_ids(db, &throwable_ids).await?;

                let throwable_config = create_throwable_config(db, items).await?;

                Ok(EventMessage::ThrowItem {
                    config: throwable_config,
                    amount,
                })
            }
            ThrowableData::Barrage {
                throwable_ids,
                amount_per_throw,
                frequency,
                amount,
            } => {
                let items = ItemModel::get_by_ids(db, &throwable_ids).await?;
                let throwable_config = create_throwable_config(db, items).await?;

                Ok(EventMessage::ThrowItemBarrage {
                    config: throwable_config,
                    amount,
                    frequency,
                    amount_per_throw,
                })
            }
        },

        EventOutcome::TriggerHotkey(data) => Ok(EventMessage::TriggerHotkey {
            hotkey_id: data.hotkey_id,
        }),
        EventOutcome::PlaySound(data) => {
            let config = SoundModel::get_by_id(db, data.sound_id)
                .await?
                .context("sound config not found")?;

            Ok(EventMessage::PlaySound { config })
        }
    }
}

async fn create_throwable_config(
    db: &DatabaseConnection,
    items: Vec<ItemModel>,
) -> anyhow::Result<ThrowableConfig> {
    // Find all the referenced sounds
    let mut futures = items
        .into_iter()
        .map(
            |item| -> BoxFuture<'_, DbResult<(ItemWithImpactSoundIds, Vec<SoundModel>)>> {
                Box::pin(async move {
                    let sounds = item.get_impact_sounds(db).await?;

                    let impact_sound_ids = sounds.iter().map(|sound| sound.id).collect();
                    Ok((
                        ItemWithImpactSoundIds {
                            item,
                            impact_sound_ids,
                        },
                        sounds,
                    ))
                })
            },
        )
        .collect::<FuturesUnordered<_>>();

    let mut items = Vec::new();
    let mut impact_sounds = Vec::new();

    while let Some(result) = futures.next().await {
        match result {
            Ok((item, mut sounds)) => {
                items.push(item);
                impact_sounds.append(&mut sounds);
            }
            Err(err) => {
                error!("error loading impact sounds: {:?}", err);
            }
        }
    }

    Ok(ThrowableConfig {
        items,
        impact_sounds,
    })
}
