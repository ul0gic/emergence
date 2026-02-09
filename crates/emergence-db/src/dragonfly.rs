//! `Dragonfly` (Redis-compatible) hot state operations.
//!
//! `Dragonfly` holds the current tick's complete world state. The World Engine
//! reads and writes to `Dragonfly` during tick execution. Key patterns follow
//! `data-schemas.md` section 10.
//!
//! # Key Patterns
//!
//! | Pattern | Type | Description |
//! |---------|------|-------------|
//! | `world:tick` | Integer | Current tick number |
//! | `world:clock` | JSON | Serialized clock state |
//! | `agent:{id}:state` | JSON | Full agent state |
//! | `location:{id}:state` | JSON | Location state with occupants |
//! | `location:{id}:messages` | List | Message board entries |
//! | `trade:{id}` | JSON | Pending trade state |

use fred::prelude::*;
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use crate::error::DbError;

/// Connection handle to a `Dragonfly` (Redis-compatible) instance.
///
/// Wraps a [`fred::prelude::Client`] and provides typed operations
/// for the key patterns defined in `data-schemas.md` section 10.
#[derive(Clone)]
pub struct DragonflyPool {
    client: Client,
}

impl DragonflyPool {
    /// Connect to `Dragonfly` at the given URL.
    ///
    /// The URL should follow the Redis URL scheme:
    /// `redis://host:port` or `redis://host:port/db`
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Config`] if the URL cannot be parsed.
    /// Returns [`DbError::Dragonfly`] if the connection fails.
    pub async fn connect(url: &str) -> Result<Self, DbError> {
        let config = Config::from_url(url)
            .map_err(|e| DbError::Config(format!("Invalid Dragonfly URL: {e}")))?;

        let client = Builder::from_config(config).build()?;
        client.init().await?;

        tracing::info!("Connected to Dragonfly");
        Ok(Self { client })
    }

    // =========================================================================
    // Generic JSON get/set/delete
    // =========================================================================

    /// Serialize `value` as JSON and store it at `key`.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Serialization`] if serialization fails.
    /// Returns [`DbError::Dragonfly`] if the write fails.
    pub async fn set_json<T: Serialize>(&self, key: &str, value: &T) -> Result<(), DbError> {
        let json = serde_json::to_string(value)?;
        let _: () = self.client.set(key, json.as_str(), None, None, false).await?;
        Ok(())
    }

    /// Read the value at `key` and deserialize from JSON.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::KeyNotFound`] if the key does not exist.
    /// Returns [`DbError::Serialization`] if deserialization fails.
    /// Returns [`DbError::Dragonfly`] if the read fails.
    pub async fn get_json<T: DeserializeOwned>(&self, key: &str) -> Result<T, DbError> {
        let value: Option<String> = self.client.get(key).await?;
        value.map_or_else(
            || Err(DbError::KeyNotFound(key.to_owned())),
            |s| Ok(serde_json::from_str(&s)?),
        )
    }

    /// Delete a key from `Dragonfly`.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Dragonfly`] if the delete fails.
    pub async fn delete(&self, key: &str) -> Result<(), DbError> {
        let _: u32 = self.client.del(key).await?;
        Ok(())
    }

    // =========================================================================
    // World State -- world:tick, world:clock
    // =========================================================================

    /// Set the current tick number (`world:tick`).
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Dragonfly`] if the write fails.
    pub async fn set_world_tick(&self, tick: u64) -> Result<(), DbError> {
        let _: () = self
            .client
            .set("world:tick", tick.to_string().as_str(), None, None, false)
            .await?;
        Ok(())
    }

    /// Get the current tick number (`world:tick`).
    ///
    /// # Errors
    ///
    /// Returns [`DbError::KeyNotFound`] if not set.
    /// Returns [`DbError::Dragonfly`] if the read fails.
    pub async fn get_world_tick(&self) -> Result<u64, DbError> {
        let value: Option<String> = self.client.get("world:tick").await?;
        value.map_or_else(
            || Err(DbError::KeyNotFound("world:tick".to_owned())),
            |s| {
                s.parse::<u64>().map_err(|e| {
                    DbError::Config(format!("world:tick is not a valid u64: {e}"))
                })
            },
        )
    }

    /// Set the serialized clock state (`world:clock`).
    ///
    /// # Errors
    ///
    /// Returns [`DbError`] if serialization or write fails.
    pub async fn set_world_clock<T: Serialize>(&self, clock: &T) -> Result<(), DbError> {
        self.set_json("world:clock", clock).await
    }

    /// Get the serialized clock state (`world:clock`).
    ///
    /// # Errors
    ///
    /// Returns [`DbError`] if not found, deserialization, or read fails.
    pub async fn get_world_clock<T: DeserializeOwned>(&self) -> Result<T, DbError> {
        self.get_json("world:clock").await
    }

    // =========================================================================
    // Agent State -- agent:{id}:state
    // =========================================================================

    /// Store the full agent state at `agent:{id}:state`.
    ///
    /// # Errors
    ///
    /// Returns [`DbError`] if serialization or write fails.
    pub async fn set_agent_state<T: Serialize>(
        &self,
        agent_id: Uuid,
        state: &T,
    ) -> Result<(), DbError> {
        let key = format!("agent:{agent_id}:state");
        self.set_json(&key, state).await
    }

    /// Get the full agent state from `agent:{id}:state`.
    ///
    /// # Errors
    ///
    /// Returns [`DbError`] if not found, deserialization, or read fails.
    pub async fn get_agent_state<T: DeserializeOwned>(
        &self,
        agent_id: Uuid,
    ) -> Result<T, DbError> {
        let key = format!("agent:{agent_id}:state");
        self.get_json(&key).await
    }

    /// Delete the agent state key.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Dragonfly`] if the delete fails.
    pub async fn delete_agent_state(&self, agent_id: Uuid) -> Result<(), DbError> {
        let key = format!("agent:{agent_id}:state");
        self.delete(&key).await
    }

    // =========================================================================
    // Location State -- location:{id}:state
    // =========================================================================

    /// Store the location state at `location:{id}:state`.
    ///
    /// # Errors
    ///
    /// Returns [`DbError`] if serialization or write fails.
    pub async fn set_location_state<T: Serialize>(
        &self,
        location_id: Uuid,
        state: &T,
    ) -> Result<(), DbError> {
        let key = format!("location:{location_id}:state");
        self.set_json(&key, state).await
    }

    /// Get the location state from `location:{id}:state`.
    ///
    /// # Errors
    ///
    /// Returns [`DbError`] if not found, deserialization, or read fails.
    pub async fn get_location_state<T: DeserializeOwned>(
        &self,
        location_id: Uuid,
    ) -> Result<T, DbError> {
        let key = format!("location:{location_id}:state");
        self.get_json(&key).await
    }

    /// Delete the location state key.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Dragonfly`] if the delete fails.
    pub async fn delete_location_state(&self, location_id: Uuid) -> Result<(), DbError> {
        let key = format!("location:{location_id}:state");
        self.delete(&key).await
    }

    // =========================================================================
    // Location Messages -- location:{id}:messages (list)
    // =========================================================================

    /// Push a message to the location message board (`location:{id}:messages`).
    ///
    /// Messages are appended to the end of the list (RPUSH).
    ///
    /// # Errors
    ///
    /// Returns [`DbError`] if serialization or write fails.
    pub async fn push_location_message<T: Serialize>(
        &self,
        location_id: Uuid,
        message: &T,
    ) -> Result<(), DbError> {
        let key = format!("location:{location_id}:messages");
        let json = serde_json::to_string(message)?;
        let _: u64 = self.client.rpush(&key, json.as_str()).await?;
        Ok(())
    }

    /// Get all messages from the location message board.
    ///
    /// Returns messages in insertion order (oldest first).
    ///
    /// # Errors
    ///
    /// Returns [`DbError`] if deserialization or read fails.
    pub async fn get_location_messages<T: DeserializeOwned>(
        &self,
        location_id: Uuid,
    ) -> Result<Vec<T>, DbError> {
        let key = format!("location:{location_id}:messages");
        let values: Vec<String> = self.client.lrange(&key, 0, -1).await?;
        let mut messages = Vec::with_capacity(values.len());
        for v in &values {
            let parsed: T = serde_json::from_str(v)?;
            messages.push(parsed);
        }
        Ok(messages)
    }

    /// Clear all messages from a location's message board.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Dragonfly`] if the delete fails.
    pub async fn clear_location_messages(&self, location_id: Uuid) -> Result<(), DbError> {
        let key = format!("location:{location_id}:messages");
        self.delete(&key).await
    }

    // =========================================================================
    // Trade State -- trade:{id}
    // =========================================================================

    /// Store a pending trade at `trade:{id}`.
    ///
    /// # Errors
    ///
    /// Returns [`DbError`] if serialization or write fails.
    pub async fn set_trade<T: Serialize>(
        &self,
        trade_id: Uuid,
        trade: &T,
    ) -> Result<(), DbError> {
        let key = format!("trade:{trade_id}");
        self.set_json(&key, trade).await
    }

    /// Get a pending trade from `trade:{id}`.
    ///
    /// # Errors
    ///
    /// Returns [`DbError`] if not found, deserialization, or read fails.
    pub async fn get_trade<T: DeserializeOwned>(&self, trade_id: Uuid) -> Result<T, DbError> {
        let key = format!("trade:{trade_id}");
        self.get_json(&key).await
    }

    /// Delete a trade (after acceptance, rejection, or expiry).
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Dragonfly`] if the delete fails.
    pub async fn delete_trade(&self, trade_id: Uuid) -> Result<(), DbError> {
        let key = format!("trade:{trade_id}");
        self.delete(&key).await
    }

    // =========================================================================
    // World Indexes -- world:agents:alive, world:agents:dead, etc.
    // =========================================================================

    /// Add an agent to the `world:agents:alive` set.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Dragonfly`] if the write fails.
    pub async fn add_alive_agent(&self, agent_id: Uuid) -> Result<(), DbError> {
        let _: u32 = self
            .client
            .sadd("world:agents:alive", agent_id.to_string().as_str())
            .await?;
        Ok(())
    }

    /// Remove an agent from `world:agents:alive` and add to `world:agents:dead`.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Dragonfly`] if the write fails.
    pub async fn mark_agent_dead(&self, agent_id: Uuid) -> Result<(), DbError> {
        let id_str = agent_id.to_string();
        let _: u32 = self
            .client
            .srem("world:agents:alive", id_str.as_str())
            .await?;
        let _: u32 = self
            .client
            .sadd("world:agents:dead", id_str.as_str())
            .await?;
        Ok(())
    }

    /// Get all living agent IDs from `world:agents:alive`.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Dragonfly`] if the read fails.
    pub async fn get_alive_agents(&self) -> Result<Vec<Uuid>, DbError> {
        let members: Vec<String> = self.client.smembers("world:agents:alive").await?;
        let mut ids = Vec::with_capacity(members.len());
        for m in &members {
            let id = m.parse::<Uuid>().map_err(|e| {
                DbError::Config(format!("Invalid UUID in world:agents:alive: {e}"))
            })?;
            ids.push(id);
        }
        Ok(ids)
    }

    /// Flush all keys from the `Dragonfly` instance.
    ///
    /// **WARNING:** This deletes all data. Only use for testing.
    ///
    /// # Errors
    ///
    /// Returns [`DbError::Dragonfly`] if the flush fails.
    pub async fn flush_all(&self) -> Result<(), DbError> {
        let _: () = self.client.flushall(false).await?;
        Ok(())
    }

    /// Return a reference to the underlying [`Client`].
    pub const fn client(&self) -> &Client {
        &self.client
    }
}
