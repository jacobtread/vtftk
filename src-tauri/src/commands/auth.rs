use crate::{
    constants::{LOCAL_SERVER_PORT, TWITCH_CLIENT_ID, TWITCH_REQUIRED_SCOPES},
    twitch::manager::TwitchManager,
};
use anyhow::Context;
use reqwest::Url;
use std::sync::Arc;
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;
use twitch_api::twitch_oauth2::{ClientId, ImplicitUserTokenBuilder};

use super::CmdResult;

/// Obtain a URL for use logging into twitch using OAuth2
#[tauri::command]
pub fn get_twitch_oauth_uri() -> String {
    let url = format!("http://localhost:{}/oauth", LOCAL_SERVER_PORT);
    let (url, _) = ImplicitUserTokenBuilder::new(
        ClientId::from_static(TWITCH_CLIENT_ID),
        Url::parse(&url).unwrap(),
    )
    .set_scopes(TWITCH_REQUIRED_SCOPES.to_vec())
    .generate_url();

    url.to_string()
}

/// Open the users default browser to a twitch OAuth URI
#[tauri::command]
pub fn open_twitch_oauth_uri(app_handle: AppHandle) -> CmdResult<()> {
    let uri = get_twitch_oauth_uri();

    app_handle
        .shell()
        .open(uri, None)
        .context("failed to open window")?;

    Ok(())
}

#[tauri::command]
pub async fn is_authenticated(state: tauri::State<'_, Arc<TwitchManager>>) -> Result<bool, ()> {
    Ok(state.is_authenticated().await)
}

#[tauri::command]
pub async fn logout(state: tauri::State<'_, Arc<TwitchManager>>) -> Result<(), ()> {
    state.reset().await;
    Ok(())
}
