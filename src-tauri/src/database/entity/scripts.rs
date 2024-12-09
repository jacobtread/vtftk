use anyhow::Context;
use sea_orm::{entity::prelude::*, ActiveValue::Set, IntoActiveModel};
use serde::{Deserialize, Serialize};

use super::{
    script_events::{ScriptEvent, ScriptEventsActiveModel, ScriptEventsColumn, ScriptEventsEntity},
    shared::DbResult,
};

// Type alias helpers for the database entity types
pub type ScriptModel = Model;
pub type ScriptEntity = Entity;
pub type ScriptActiveModel = ActiveModel;
pub type ScriptColumn = Column;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "scripts")]
pub struct Model {
    /// Unique ID for the sound
    #[sea_orm(primary_key)]
    pub id: Uuid,
    /// Whether the script is enabled and runnable
    pub enabled: bool,
    /// Name of the script
    pub name: String,
    /// The actual script contents
    pub script: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    /// Script can have many events
    #[sea_orm(has_many = "super::script_events::Entity")]
    Events,
}

impl Related<super::script_events::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Events.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Deserialize)]
pub struct CreateScript {
    pub enabled: bool,
    pub name: String,
    pub script: String,
    pub events: Vec<ScriptEvent>,
}

#[derive(Default, Deserialize)]
pub struct UpdateScript {
    pub enabled: Option<bool>,
    pub name: Option<String>,
    pub script: Option<String>,
    pub events: Option<Vec<ScriptEvent>>,
}

impl Model {
    /// Create a new script
    pub async fn create<C>(db: &C, create: CreateScript) -> anyhow::Result<Model>
    where
        C: ConnectionTrait + Send + 'static,
    {
        let id = Uuid::new_v4();
        let active_model = ActiveModel {
            id: Set(id),
            enabled: Set(create.enabled),
            name: Set(create.name),
            script: Set(create.script),
        };

        Entity::insert(active_model)
            .exec_without_returning(db)
            .await?;

        let model = Self::get_by_id(db, id)
            .await?
            .context("model was not inserted")?;

        model.set_script_events(db, &create.events).await?;

        Ok(model)
    }

    /// Find a specific script by ID
    pub async fn get_by_id<C>(db: &C, id: Uuid) -> DbResult<Option<Self>>
    where
        C: ConnectionTrait + Send + 'static,
    {
        Entity::find_by_id(id).one(db).await
    }

    /// Find a script by the event its subscribed to filters to only enabled
    pub async fn get_by_event<C>(db: &C, script_event: ScriptEvent) -> DbResult<Vec<Self>>
    where
        C: ConnectionTrait + Send + 'static,
    {
        Entity::find()
            .inner_join(super::script_events::Entity)
            .filter(ScriptEventsColumn::Event.eq(script_event))
            .all(db)
            .await
    }

    /// Find all script
    pub async fn all<C>(db: &C) -> DbResult<Vec<Self>>
    where
        C: ConnectionTrait + Send + 'static,
    {
        Entity::find().all(db).await
    }

    /// Update the current script
    pub async fn update<C>(self, db: &C, data: UpdateScript) -> DbResult<Self>
    where
        C: ConnectionTrait + Send + 'static,
    {
        let mut this = self.into_active_model();

        if let Some(enabled) = data.enabled {
            this.enabled = Set(enabled);
        }

        if let Some(name) = data.name {
            this.name = Set(name);
        }

        if let Some(script) = data.script {
            this.script = Set(script);
        }

        let this = this.update(db).await?;

        if let Some(events) = data.events {
            this.set_script_events(db, &events).await?;
        }

        Ok(this)
    }

    pub async fn set_script_events<C>(&self, db: &C, script_events: &[ScriptEvent]) -> DbResult<()>
    where
        C: ConnectionTrait + Send + 'static,
    {
        // Delete any impact sounds not in the provided list
        ScriptEventsEntity::delete_many()
            .filter(
                ScriptEventsColumn::ScriptId
                    .eq(self.id)
                    .and(ScriptEventsColumn::Event.is_not_in(script_events.iter().copied())),
            )
            .exec(db)
            .await?;

        self.append_script_events(db, script_events).await?;

        Ok(())
    }

    pub async fn append_script_events<C>(
        &self,
        db: &C,
        script_events: &[ScriptEvent],
    ) -> DbResult<()>
    where
        C: ConnectionTrait + Send + 'static,
    {
        // Insert the new connections
        ScriptEventsEntity::insert_many(script_events.iter().map(|script_event| {
            ScriptEventsActiveModel {
                script_id: Set(self.id),
                event: Set(*script_event),
            }
        }))
        // Ignore already existing connections
        .on_conflict_do_nothing()
        .exec(db)
        .await?;

        Ok(())
    }
}
