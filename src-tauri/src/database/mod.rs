use std::path::Path;

use anyhow::Context;
use chrono::Days;
use chrono::Utc;
use entity::CommandExecutionModel;
use entity::CommandLogsModel;
use entity::EventExecutionModel;
use entity::EventLogsModel;
use log::warn;
use migration::Migrator;
use sea_orm::Database;
use sea_orm::DatabaseConnection;
use sea_orm_migration::MigratorTrait;
use tokio::fs::{create_dir_all, File};

use crate::state::app_data::AppDataStore;

pub mod entity;
mod migration;

/// Connects to the SQLite database at the provided path, creating a
/// new database file if none exist
pub async fn connect_database(path: &Path) -> anyhow::Result<DatabaseConnection> {
    if !path.exists() {
        let parent = path.parent().context("database path invalid")?;
        create_dir_all(parent)
            .await
            .context("create database path")?;

        File::create(path).await?;
    }

    let path = path.to_str().context("invalid db path")?;

    let path = format!("sqlite://{path}");

    let options = sea_orm::ConnectOptions::new(path);
    let db = Database::connect(options).await?;

    if let Err(err) = Migrator::up(&db, None).await {
        warn!("failed to apply/check database migrations: {:?}", err);
        // TODO: Check for applied forward migrations, these are not always failing changes
    }

    Ok(db)
}

pub async fn clean_old_data(db: DatabaseConnection, app_data: AppDataStore) -> anyhow::Result<()> {
    let app_data = app_data.read().await;
    let main_config = &app_data.main_config;

    let now = Utc::now();

    // Clean logs
    if main_config.clean_logs {
        let clean_logs_date = now
            .checked_sub_days(Days::new(main_config.clean_logs_days))
            .context("system time is incorrect")?;

        EventLogsModel::delete_before(&db, clean_logs_date).await?;
        CommandLogsModel::delete_before(&db, clean_logs_date).await?;
    }

    // Clean executions
    if main_config.clean_executions {
        let clean_executions_date = now
            .checked_sub_days(Days::new(main_config.clean_executions_days))
            .context("system time is incorrect")?;

        CommandExecutionModel::delete_before(&db, clean_executions_date).await?;
        EventExecutionModel::delete_before(&db, clean_executions_date).await?;
    }

    Ok(())
}
