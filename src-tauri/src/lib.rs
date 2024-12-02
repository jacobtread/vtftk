use anyhow::Context;
use event_processing::handle_twitch_events;
use events::{EventMessage, EventRecvHandle};
use log::{debug, error};
use script::{kv::KVStore, runtime::create_script_executor};
use state::{app_data::AppDataStore, runtime_app_data::RuntimeAppDataStore};
use std::sync::Arc;
use tauri::{App, Manager};
use tokio::sync::broadcast;
use twitch::manager::TwitchManager;
use twitch_api::{
    twitch_oauth2::{AccessToken, UserToken},
    HelixClient,
};

mod commands;
mod constants;
mod event_processing;
mod events;
mod http;
mod script;
mod state;
mod twitch;

/// Prevent slow changes from macro by using a separate entrypoint
/// from the macro
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        .setup(move |app| {
            let handle = app.handle().clone();

            let (twitch_manager, twitch_event_rx) =
                TwitchManager::new(HelixClient::default(), handle.clone());
            let (event_tx, event_rx) = create_event_channel();

            let app_data = tauri::async_runtime::block_on(load_app_data(app))
                .expect("failed to load app data");
            let kv_store =
                tauri::async_runtime::block_on(load_kv(app)).expect("failed to load app data");

            let runtime_app_data = RuntimeAppDataStore::new(handle.clone());

            let script_handle = create_script_executor();

            // Provide app data and runtime app data stores
            app.manage(app_data.clone());
            app.manage(runtime_app_data.clone());

            // Provide access to twitch manager and event sender
            app.manage(event_tx.clone());
            app.manage(twitch_manager.clone());

            // Provide access to script running and
            app.manage(script_handle.clone());

            // Attempt to authenticate with twitch using the saved token
            _ = tauri::async_runtime::spawn(attempt_twitch_auth_existing_token(
                app_data.clone(),
                twitch_manager.clone(),
            ));

            // Handle scripting events
            script::events::init_script_event_handling(
                app_data.clone(),
                kv_store,
                twitch_manager.clone(),
            );

            // Handle events triggered by twitch
            _ = tauri::async_runtime::spawn(handle_twitch_events(
                app_data.clone(),
                twitch_manager.clone(),
                twitch_event_rx,
                event_tx.clone(),
                script_handle,
            ));

            // Run HTTP server
            _ = tauri::async_runtime::spawn(async move {
                _ = http::server::start(
                    event_rx,
                    handle,
                    twitch_manager,
                    app_data,
                    runtime_app_data,
                )
                .await;
            });

            // TODO: Start server and block until a channel reports back that the server started?
            // store server task in a state variable to allow attempting restart within app
            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::auth::get_twitch_oauth_uri,
            commands::auth::is_authenticated,
            commands::auth::open_twitch_oauth_uri,
            commands::auth::logout,
            commands::calibration::set_calibration_step,
            commands::test::test_throw,
            commands::test::test_throw_barrage,
            commands::test::test_sound,
            commands::test::test_get_script_events,
            commands::data::get_app_data,
            commands::data::get_runtime_app_data,
            commands::data::set_app_data,
            commands::data::upload_file,
            commands::data::update_hotkeys,
            commands::data::get_overlay_url,
            commands::twitch::get_redeems_list,
            commands::twitch::refresh_redeems_list,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn create_event_channel() -> (broadcast::Sender<EventMessage>, EventRecvHandle) {
    let (event_tx, rx) = broadcast::channel(10);
    let event_rx = EventRecvHandle(rx);

    (event_tx, event_rx)
}

async fn load_app_data(app: &App) -> anyhow::Result<AppDataStore> {
    let app_data_path = app
        .path()
        .app_data_dir()
        .context("failed to get app data dir")?;
    let app_data_file = app_data_path.join("data.json");

    debug!("app data path: {:?}", app_data_path);

    let app_data = AppDataStore::load(app_data_file)
        .await
        .context("failed to load app data")?;

    Ok(app_data)
}

async fn load_kv(app: &App) -> anyhow::Result<KVStore> {
    let app_data_path = app
        .path()
        .app_data_dir()
        .context("failed to get app data dir")?;
    let kv_file = app_data_path.join("kv_data.json");

    debug!("app data path: {:?}", app_data_path);

    let app_data = KVStore::load(kv_file)
        .await
        .context("failed to load kv data")?;

    Ok(app_data)
}

/// Attempts to authenticate with twitch using an existing access token
async fn attempt_twitch_auth_existing_token(
    app_data_store: AppDataStore,
    twitch_manager: Arc<TwitchManager>,
) {
    let app_data = app_data_store.read().await;

    let access_token = match app_data.twitch_config.access_token.as_ref() {
        Some(value) => value,
        None => return,
    };

    let access_token = AccessToken::from(access_token.as_str());

    // Create user token (Validates it with the twitch backend)
    let user_token = match UserToken::from_existing(
        &twitch_manager.helix_client,
        access_token,
        None,
        None,
    )
    .await
    {
        Ok(value) => value,
        Err(err) => {
            error!("stored access token is invalid: {}", err);

            // Drop read access to app data
            drop(app_data);

            // Clear twitch token
            _ = app_data_store
                .write(|app_data| {
                    app_data.twitch_config.access_token = None;
                })
                .await;

            return;
        }
    };

    twitch_manager.set_authenticated(user_token).await;
}
