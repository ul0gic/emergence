//! Private and secret communication routing for the Emergence simulation.
//!
//! Implements task 6.6.1: message visibility levels (public, whisper, conspire,
//! location announcement) with routing logic that respects fog of war. Only
//! agents who should see a message can retrieve it.
//!
//! # Architecture
//!
//! The [`MessageRouter`] is the central dispatcher. It stores all messages
//! for the current simulation window and answers queries like "what messages
//! should agent X see at tick T?" without leaking private channels.
//!
//! # Eavesdropping
//!
//! Agents with high curiosity at the same location as a whisper have a small
//! probabilistic chance of intercepting the message. The formula is:
//!
//! `chance = min(curiosity * 0.05, 0.15)`
//!
//! This is rolled per eligible agent per whisper message.

use std::collections::BTreeMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use emergence_types::{AgentId, LocationId};

use crate::error::AgentError;

// ---------------------------------------------------------------------------
// MessageVisibility
// ---------------------------------------------------------------------------

/// Determines who can receive a message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageVisibility {
    /// Everyone at the location can see the message (existing broadcast behavior).
    Public,
    /// Only the target agent receives the message.
    Whisper {
        /// The sole intended recipient.
        target: AgentId,
    },
    /// Only the listed agents receive the message (conspiracy channel).
    Conspire {
        /// The agents in the secret group.
        group: Vec<AgentId>,
    },
    /// Persists at a location for newcomers to see (used by propaganda later).
    LocationAnnouncement,
}

// ---------------------------------------------------------------------------
// PrivateMessage
// ---------------------------------------------------------------------------

/// A message routed through the communication system with visibility controls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateMessage {
    /// Unique message identifier.
    pub id: Uuid,
    /// The agent who sent the message.
    pub sender: AgentId,
    /// The tick when the message was sent.
    pub tick: u64,
    /// The text content of the message.
    pub content: String,
    /// Who can see this message.
    pub visibility: MessageVisibility,
    /// The location where the message was sent (required for routing).
    pub location: Option<LocationId>,
}

// ---------------------------------------------------------------------------
// CommunicationStats
// ---------------------------------------------------------------------------

/// Aggregate statistics about the communication system.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommunicationStats {
    /// Total messages across all visibility types.
    pub total_messages: u32,
    /// Number of public messages.
    pub public_count: u32,
    /// Number of whisper messages.
    pub whisper_count: u32,
    /// Number of conspire messages.
    pub conspire_count: u32,
    /// Number of messages intercepted by eavesdroppers.
    pub eavesdropped_count: u32,
}

// ---------------------------------------------------------------------------
// MessageRouter
// ---------------------------------------------------------------------------

/// Routes messages based on visibility and provides query interfaces.
///
/// The router stores all messages and answers per-agent, per-location,
/// and per-tick queries. It enforces visibility rules so that agents
/// never see messages they should not have access to.
#[derive(Debug, Clone)]
pub struct MessageRouter {
    /// All messages, keyed by their unique ID.
    messages: BTreeMap<Uuid, PrivateMessage>,
    /// Running statistics.
    stats: CommunicationStats,
}

impl MessageRouter {
    /// Create a new empty message router.
    pub const fn new() -> Self {
        Self {
            messages: BTreeMap::new(),
            stats: CommunicationStats {
                total_messages: 0,
                public_count: 0,
                whisper_count: 0,
                conspire_count: 0,
                eavesdropped_count: 0,
            },
        }
    }

    /// Send a message, routing it based on visibility.
    ///
    /// The message is stored and statistics are updated.
    pub fn send_message(&mut self, message: PrivateMessage) -> Result<(), AgentError> {
        // Update stats based on visibility type.
        match &message.visibility {
            MessageVisibility::Public | MessageVisibility::LocationAnnouncement => {
                self.stats.public_count = self
                    .stats
                    .public_count
                    .checked_add(1)
                    .ok_or_else(|| AgentError::ArithmeticOverflow {
                        context: String::from("public message count overflow"),
                    })?;
            }
            MessageVisibility::Whisper { .. } => {
                self.stats.whisper_count = self
                    .stats
                    .whisper_count
                    .checked_add(1)
                    .ok_or_else(|| AgentError::ArithmeticOverflow {
                        context: String::from("whisper message count overflow"),
                    })?;
            }
            MessageVisibility::Conspire { .. } => {
                self.stats.conspire_count = self
                    .stats
                    .conspire_count
                    .checked_add(1)
                    .ok_or_else(|| AgentError::ArithmeticOverflow {
                        context: String::from("conspire message count overflow"),
                    })?;
            }
        }

        self.stats.total_messages = self
            .stats
            .total_messages
            .checked_add(1)
            .ok_or_else(|| AgentError::ArithmeticOverflow {
                context: String::from("total message count overflow"),
            })?;

        self.messages.insert(message.id, message);
        Ok(())
    }

    /// Get all messages an agent should receive at a given tick.
    ///
    /// Returns messages matching any of these criteria:
    /// - Public messages at the agent's location for the given tick
    /// - Whisper messages where the agent is the target
    /// - Conspire messages where the agent is in the group
    /// - `LocationAnnouncement` messages at the agent's location (all ticks)
    pub fn get_messages_for_agent(
        &self,
        agent_id: AgentId,
        agent_location: LocationId,
        tick: u64,
    ) -> Vec<&PrivateMessage> {
        self.messages
            .values()
            .filter(|msg| {
                match &msg.visibility {
                    MessageVisibility::Public => {
                        msg.tick == tick
                            && msg.location.is_some_and(|loc| loc == agent_location)
                    }
                    MessageVisibility::Whisper { target } => {
                        msg.tick == tick && *target == agent_id
                    }
                    MessageVisibility::Conspire { group } => {
                        msg.tick == tick && group.contains(&agent_id)
                    }
                    MessageVisibility::LocationAnnouncement => {
                        // Location announcements persist -- visible regardless of tick.
                        msg.location.is_some_and(|loc| loc == agent_location)
                    }
                }
            })
            .collect()
    }

    /// Get only public messages at a specific location for a given tick.
    pub fn get_public_messages_at_location(
        &self,
        location: LocationId,
        tick: u64,
    ) -> Vec<&PrivateMessage> {
        self.messages
            .values()
            .filter(|msg| {
                msg.tick == tick
                    && msg.location.is_some_and(|loc| loc == location)
                    && matches!(msg.visibility, MessageVisibility::Public)
            })
            .collect()
    }

    /// Get the private message history between two specific agents.
    ///
    /// Returns whisper messages sent from either agent to the other,
    /// ordered by their position in the `BTreeMap` (UUID v7 = time-ordered).
    pub fn get_private_messages_between(
        &self,
        agent_a: AgentId,
        agent_b: AgentId,
    ) -> Vec<&PrivateMessage> {
        self.messages
            .values()
            .filter(|msg| {
                if let MessageVisibility::Whisper { target } = &msg.visibility {
                    (msg.sender == agent_a && *target == agent_b)
                        || (msg.sender == agent_b && *target == agent_a)
                } else {
                    false
                }
            })
            .collect()
    }

    /// Get conspire messages for a specific group of agents.
    ///
    /// A message matches if its conspire group contains all members
    /// of the queried group (i.e., the queried group is a subset of
    /// the message's group).
    pub fn get_conspire_messages_for_group(
        &self,
        group: &[AgentId],
    ) -> Vec<&PrivateMessage> {
        self.messages
            .values()
            .filter(|msg| {
                if let MessageVisibility::Conspire { group: msg_group } = &msg.visibility {
                    group.iter().all(|member| msg_group.contains(member))
                } else {
                    false
                }
            })
            .collect()
    }

    /// Return aggregate message counts by visibility type.
    pub const fn message_count_by_visibility(&self) -> &CommunicationStats {
        &self.stats
    }

    /// Purge messages older than `retention_ticks` ticks relative to `current_tick`.
    ///
    /// `LocationAnnouncement` messages are exempt -- they persist indefinitely.
    pub fn clear_old_messages(&mut self, current_tick: u64, retention_ticks: u64) {
        let cutoff = current_tick.saturating_sub(retention_ticks);

        self.messages.retain(|_id, msg| {
            // Location announcements never expire.
            if matches!(msg.visibility, MessageVisibility::LocationAnnouncement) {
                return true;
            }
            // Keep messages at or after the cutoff tick.
            msg.tick >= cutoff
        });
    }

    /// Probabilistic eavesdrop check for whisper messages.
    ///
    /// An agent at the same location as a whisper has a chance of
    /// intercepting it based on their curiosity trait. The probability
    /// is `min(curiosity * 0.05, 0.15)`, where curiosity is a `Decimal`
    /// in the range 0.0 to 1.0.
    ///
    /// `roll` is a value in `0..10000` from the caller's RNG. This keeps
    /// the function deterministic and testable.
    ///
    /// Returns `Some(&PrivateMessage)` if the eavesdrop succeeds, `None` otherwise.
    pub fn eavesdrop_check(
        &mut self,
        message_id: Uuid,
        eavesdropper_id: AgentId,
        eavesdropper_location: LocationId,
        curiosity: Decimal,
        roll: u32,
    ) -> Option<&PrivateMessage> {
        // Look up the message.
        let msg = self.messages.get(&message_id)?;

        // Only whispers can be eavesdropped.
        let target = match &msg.visibility {
            MessageVisibility::Whisper { target } => *target,
            _ => return None,
        };

        // The eavesdropper must be at the same location as the whisper.
        let msg_location = msg.location?;
        if msg_location != eavesdropper_location {
            return None;
        }

        // The eavesdropper cannot be the sender or the intended target.
        if eavesdropper_id == msg.sender || eavesdropper_id == target {
            return None;
        }

        // Compute eavesdrop probability: min(curiosity * 0.05, 0.15)
        let factor = Decimal::new(5, 2); // 0.05
        let chance_decimal = curiosity.saturating_mul(factor);
        let max_chance = Decimal::new(15, 2); // 0.15
        let clamped_chance = if chance_decimal > max_chance {
            max_chance
        } else {
            chance_decimal
        };

        // Convert to per-10000 threshold for roll comparison.
        let threshold_val = compute_threshold_per_10000(clamped_chance);

        if roll < threshold_val {
            // Eavesdrop succeeded. Increment the counter.
            // We use saturating_add here since this is a stats counter.
            self.stats.eavesdropped_count = self.stats.eavesdropped_count.saturating_add(1);
            self.messages.get(&message_id)
        } else {
            None
        }
    }

    /// Get total number of stored messages.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
}

impl Default for MessageRouter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Convert a chance `Decimal` (0.0 to 0.15) to a per-10000 threshold as `u32`.
///
/// For example, 0.05 becomes 500, 0.15 becomes 1500.
fn compute_threshold_per_10000(chance: Decimal) -> u32 {
    // chance * 10000, then take integer part.
    let scaled = chance.saturating_mul(Decimal::from(10000));
    let truncated = scaled.trunc();

    // Extract the integer value. For small positive decimals this is safe.
    // We use checked conversion to avoid any overflow.
    if truncated.is_sign_negative() {
        return 0;
    }

    // The mantissa of a truncated Decimal with value N has mantissa = N * 10^scale.
    // Dividing mantissa by 10^scale gives us N.
    let scale = truncated.scale();
    let mantissa = truncated.mantissa();
    if mantissa < 0 {
        return 0;
    }
    let mantissa_u128 = mantissa.unsigned_abs();

    let divisor: u128 = 10_u128.checked_pow(scale).unwrap_or(1);
    let integer_val = mantissa_u128.checked_div(divisor).unwrap_or(0);

    // The maximum threshold is 1500 (for 0.15 * 10000), well within u32.
    u32::try_from(integer_val).unwrap_or(u32::MAX)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use uuid::Uuid;

    use emergence_types::{AgentId, LocationId};

    use super::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_public_message(sender: AgentId, location: LocationId, tick: u64) -> PrivateMessage {
        PrivateMessage {
            id: Uuid::now_v7(),
            sender,
            tick,
            content: String::from("hello everyone"),
            visibility: MessageVisibility::Public,
            location: Some(location),
        }
    }

    fn make_whisper(
        sender: AgentId,
        target: AgentId,
        location: LocationId,
        tick: u64,
    ) -> PrivateMessage {
        PrivateMessage {
            id: Uuid::now_v7(),
            sender,
            tick,
            content: String::from("secret whisper"),
            visibility: MessageVisibility::Whisper { target },
            location: Some(location),
        }
    }

    fn make_conspire(
        sender: AgentId,
        group: Vec<AgentId>,
        location: LocationId,
        tick: u64,
    ) -> PrivateMessage {
        PrivateMessage {
            id: Uuid::now_v7(),
            sender,
            tick,
            content: String::from("conspiracy plan"),
            visibility: MessageVisibility::Conspire { group },
            location: Some(location),
        }
    }

    fn make_announcement(
        sender: AgentId,
        location: LocationId,
        tick: u64,
    ) -> PrivateMessage {
        PrivateMessage {
            id: Uuid::now_v7(),
            sender,
            tick,
            content: String::from("location announcement"),
            visibility: MessageVisibility::LocationAnnouncement,
            location: Some(location),
        }
    }

    // -----------------------------------------------------------------------
    // 1. Public routing
    // -----------------------------------------------------------------------

    #[test]
    fn public_message_visible_to_agents_at_same_location() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let receiver = AgentId::new();
        let location = LocationId::new();

        let msg = make_public_message(sender, location, 10);
        let result = router.send_message(msg);
        assert!(result.is_ok());

        let messages = router.get_messages_for_agent(receiver, location, 10);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages.first().map(|m| &m.content), Some(&String::from("hello everyone")));
    }

    #[test]
    fn public_message_not_visible_at_different_location() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let receiver = AgentId::new();
        let location_a = LocationId::new();
        let location_b = LocationId::new();

        let msg = make_public_message(sender, location_a, 10);
        let result = router.send_message(msg);
        assert!(result.is_ok());

        let messages = router.get_messages_for_agent(receiver, location_b, 10);
        assert!(messages.is_empty());
    }

    #[test]
    fn public_message_not_visible_at_different_tick() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let receiver = AgentId::new();
        let location = LocationId::new();

        let msg = make_public_message(sender, location, 10);
        let result = router.send_message(msg);
        assert!(result.is_ok());

        let messages = router.get_messages_for_agent(receiver, location, 11);
        assert!(messages.is_empty());
    }

    // -----------------------------------------------------------------------
    // 2. Whisper routing
    // -----------------------------------------------------------------------

    #[test]
    fn whisper_only_reaches_target() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let target = AgentId::new();
        let bystander = AgentId::new();
        let location = LocationId::new();

        let msg = make_whisper(sender, target, location, 10);
        let result = router.send_message(msg);
        assert!(result.is_ok());

        // Target receives the whisper.
        let target_msgs = router.get_messages_for_agent(target, location, 10);
        assert_eq!(target_msgs.len(), 1);

        // Bystander does NOT receive the whisper.
        let bystander_msgs = router.get_messages_for_agent(bystander, location, 10);
        assert!(bystander_msgs.is_empty());
    }

    #[test]
    fn whisper_not_visible_to_sender_in_query() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let target = AgentId::new();
        let location = LocationId::new();

        let msg = make_whisper(sender, target, location, 10);
        let result = router.send_message(msg);
        assert!(result.is_ok());

        // Sender does not receive their own whisper back (they already know).
        let sender_msgs = router.get_messages_for_agent(sender, location, 10);
        assert!(sender_msgs.is_empty());
    }

    // -----------------------------------------------------------------------
    // 3. Conspire routing
    // -----------------------------------------------------------------------

    #[test]
    fn conspire_only_reaches_group_members() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let member_a = AgentId::new();
        let member_b = AgentId::new();
        let outsider = AgentId::new();
        let location = LocationId::new();

        let msg = make_conspire(
            sender,
            vec![sender, member_a, member_b],
            location,
            10,
        );
        let result = router.send_message(msg);
        assert!(result.is_ok());

        // Group members see it.
        let a_msgs = router.get_messages_for_agent(member_a, location, 10);
        assert_eq!(a_msgs.len(), 1);
        let b_msgs = router.get_messages_for_agent(member_b, location, 10);
        assert_eq!(b_msgs.len(), 1);
        let sender_msgs = router.get_messages_for_agent(sender, location, 10);
        assert_eq!(sender_msgs.len(), 1);

        // Outsider does NOT see it.
        let outsider_msgs = router.get_messages_for_agent(outsider, location, 10);
        assert!(outsider_msgs.is_empty());
    }

    // -----------------------------------------------------------------------
    // 4. Location announcements
    // -----------------------------------------------------------------------

    #[test]
    fn location_announcement_persists_across_ticks() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let newcomer = AgentId::new();
        let location = LocationId::new();

        let msg = make_announcement(sender, location, 5);
        let result = router.send_message(msg);
        assert!(result.is_ok());

        // Visible at tick 5.
        let msgs_t5 = router.get_messages_for_agent(newcomer, location, 5);
        assert_eq!(msgs_t5.len(), 1);

        // Still visible at tick 100 (newcomer arrives later).
        let msgs_t100 = router.get_messages_for_agent(newcomer, location, 100);
        assert_eq!(msgs_t100.len(), 1);
    }

    #[test]
    fn location_announcement_not_visible_at_different_location() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let receiver = AgentId::new();
        let location_a = LocationId::new();
        let location_b = LocationId::new();

        let msg = make_announcement(sender, location_a, 5);
        let result = router.send_message(msg);
        assert!(result.is_ok());

        let msgs = router.get_messages_for_agent(receiver, location_b, 5);
        assert!(msgs.is_empty());
    }

    // -----------------------------------------------------------------------
    // 5. Old message cleanup
    // -----------------------------------------------------------------------

    #[test]
    fn clear_old_messages_removes_expired() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let location = LocationId::new();

        // Message at tick 5.
        let old_msg = make_public_message(sender, location, 5);
        let result = router.send_message(old_msg);
        assert!(result.is_ok());

        // Message at tick 15.
        let new_msg = make_public_message(sender, location, 15);
        let result = router.send_message(new_msg);
        assert!(result.is_ok());

        assert_eq!(router.message_count(), 2);

        // Purge messages older than 5 ticks from tick 15. Cutoff = 10.
        router.clear_old_messages(15, 5);

        // Only the message at tick 15 survives (tick 5 < cutoff 10).
        assert_eq!(router.message_count(), 1);
    }

    #[test]
    fn clear_old_messages_preserves_announcements() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let location = LocationId::new();

        // Announcement at tick 1.
        let announcement = make_announcement(sender, location, 1);
        let result = router.send_message(announcement);
        assert!(result.is_ok());

        // Public message at tick 2.
        let public = make_public_message(sender, location, 2);
        let result = router.send_message(public);
        assert!(result.is_ok());

        // Purge with retention 0 from tick 100 -- everything old is purged.
        router.clear_old_messages(100, 0);

        // Announcement survives, public does not.
        assert_eq!(router.message_count(), 1);
    }

    // -----------------------------------------------------------------------
    // 6. Message history between agents
    // -----------------------------------------------------------------------

    #[test]
    fn private_messages_between_two_agents() {
        let mut router = MessageRouter::new();
        let alice = AgentId::new();
        let bob = AgentId::new();
        let carol = AgentId::new();
        let location = LocationId::new();

        // Alice -> Bob whisper.
        let msg1 = make_whisper(alice, bob, location, 10);
        let result = router.send_message(msg1);
        assert!(result.is_ok());

        // Bob -> Alice whisper.
        let msg2 = make_whisper(bob, alice, location, 11);
        let result = router.send_message(msg2);
        assert!(result.is_ok());

        // Alice -> Carol whisper (should not appear).
        let msg3 = make_whisper(alice, carol, location, 12);
        let result = router.send_message(msg3);
        assert!(result.is_ok());

        let history = router.get_private_messages_between(alice, bob);
        assert_eq!(history.len(), 2);
    }

    // -----------------------------------------------------------------------
    // 7. Conspire group messages
    // -----------------------------------------------------------------------

    #[test]
    fn conspire_messages_for_group() {
        let mut router = MessageRouter::new();
        let agent_a = AgentId::new();
        let agent_b = AgentId::new();
        let agent_c = AgentId::new();
        let location = LocationId::new();

        // Conspire between A and B.
        let msg1 = make_conspire(agent_a, vec![agent_a, agent_b], location, 10);
        let result = router.send_message(msg1);
        assert!(result.is_ok());

        // Conspire between A, B, and C.
        let msg2 = make_conspire(agent_a, vec![agent_a, agent_b, agent_c], location, 11);
        let result = router.send_message(msg2);
        assert!(result.is_ok());

        // Querying for group [A, B] should return both messages
        // (the first is exactly [A,B], the second is a superset).
        let group_msgs = router.get_conspire_messages_for_group(&[agent_a, agent_b]);
        assert_eq!(group_msgs.len(), 2);

        // Querying for group [A, B, C] should return only the second.
        let group_msgs = router.get_conspire_messages_for_group(&[agent_a, agent_b, agent_c]);
        assert_eq!(group_msgs.len(), 1);
    }

    // -----------------------------------------------------------------------
    // 8. Stats tracking
    // -----------------------------------------------------------------------

    #[test]
    fn stats_track_message_types() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let target = AgentId::new();
        let location = LocationId::new();

        let _ = router.send_message(make_public_message(sender, location, 1));
        let _ = router.send_message(make_public_message(sender, location, 2));
        let _ = router.send_message(make_whisper(sender, target, location, 3));
        let _ = router.send_message(make_conspire(sender, vec![sender, target], location, 4));
        let _ = router.send_message(make_announcement(sender, location, 5));

        let stats = router.message_count_by_visibility();
        assert_eq!(stats.total_messages, 5);
        // Public + LocationAnnouncement share the public counter.
        assert_eq!(stats.public_count, 3);
        assert_eq!(stats.whisper_count, 1);
        assert_eq!(stats.conspire_count, 1);
    }

    // -----------------------------------------------------------------------
    // 9. Eavesdrop probability
    // -----------------------------------------------------------------------

    #[test]
    fn eavesdrop_zero_curiosity_always_fails() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let target = AgentId::new();
        let eavesdropper = AgentId::new();
        let location = LocationId::new();

        let msg = make_whisper(sender, target, location, 10);
        let msg_id = msg.id;
        let result = router.send_message(msg);
        assert!(result.is_ok());

        // With curiosity 0, chance is 0. Roll of 0 should still fail
        // because threshold is 0 and roll < 0 is false.
        let intercepted = router.eavesdrop_check(
            msg_id,
            eavesdropper,
            location,
            Decimal::ZERO,
            0,
        );
        assert!(intercepted.is_none());
    }

    #[test]
    fn eavesdrop_max_curiosity_capped_at_15_percent() {
        let threshold = compute_threshold_per_10000(Decimal::new(15, 2));
        assert_eq!(threshold, 1500);

        // Even with curiosity > 1.0, the cap is 0.15 = 1500/10000.
        let curiosity = Decimal::new(10, 0); // 10.0, way above 1.0
        let raw = curiosity.saturating_mul(Decimal::new(5, 2));
        let capped = if raw > Decimal::new(15, 2) {
            Decimal::new(15, 2)
        } else {
            raw
        };
        let threshold_capped = compute_threshold_per_10000(capped);
        assert_eq!(threshold_capped, 1500);
    }

    #[test]
    fn eavesdrop_succeeds_with_low_roll() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let target = AgentId::new();
        let eavesdropper = AgentId::new();
        let location = LocationId::new();

        let msg = make_whisper(sender, target, location, 10);
        let msg_id = msg.id;
        let result = router.send_message(msg);
        assert!(result.is_ok());

        // Curiosity = 1.0, chance = 0.05, threshold = 500/10000.
        // Roll of 100 < 500 => eavesdrop succeeds.
        let intercepted = router.eavesdrop_check(
            msg_id,
            eavesdropper,
            location,
            Decimal::ONE,
            100,
        );
        assert!(intercepted.is_some());
        assert_eq!(router.message_count_by_visibility().eavesdropped_count, 1);
    }

    #[test]
    fn eavesdrop_fails_with_high_roll() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let target = AgentId::new();
        let eavesdropper = AgentId::new();
        let location = LocationId::new();

        let msg = make_whisper(sender, target, location, 10);
        let msg_id = msg.id;
        let result = router.send_message(msg);
        assert!(result.is_ok());

        // Curiosity = 1.0, threshold = 500. Roll of 5000 >= 500 => fails.
        let intercepted = router.eavesdrop_check(
            msg_id,
            eavesdropper,
            location,
            Decimal::ONE,
            5000,
        );
        assert!(intercepted.is_none());
    }

    #[test]
    fn eavesdrop_wrong_location_fails() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let target = AgentId::new();
        let eavesdropper = AgentId::new();
        let location_a = LocationId::new();
        let location_b = LocationId::new();

        let msg = make_whisper(sender, target, location_a, 10);
        let msg_id = msg.id;
        let result = router.send_message(msg);
        assert!(result.is_ok());

        // Eavesdropper at different location.
        let intercepted = router.eavesdrop_check(
            msg_id,
            eavesdropper,
            location_b,
            Decimal::ONE,
            0,
        );
        assert!(intercepted.is_none());
    }

    #[test]
    fn eavesdrop_target_cannot_eavesdrop_own_message() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let target = AgentId::new();
        let location = LocationId::new();

        let msg = make_whisper(sender, target, location, 10);
        let msg_id = msg.id;
        let result = router.send_message(msg);
        assert!(result.is_ok());

        // The target is not an eavesdropper -- they receive the message normally.
        let intercepted = router.eavesdrop_check(
            msg_id,
            target,
            location,
            Decimal::ONE,
            0,
        );
        assert!(intercepted.is_none());
    }

    #[test]
    fn eavesdrop_sender_cannot_eavesdrop_own_message() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let target = AgentId::new();
        let location = LocationId::new();

        let msg = make_whisper(sender, target, location, 10);
        let msg_id = msg.id;
        let result = router.send_message(msg);
        assert!(result.is_ok());

        let intercepted = router.eavesdrop_check(
            msg_id,
            sender,
            location,
            Decimal::ONE,
            0,
        );
        assert!(intercepted.is_none());
    }

    #[test]
    fn eavesdrop_on_public_message_fails() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let eavesdropper = AgentId::new();
        let location = LocationId::new();

        let msg = make_public_message(sender, location, 10);
        let msg_id = msg.id;
        let result = router.send_message(msg);
        assert!(result.is_ok());

        // Cannot eavesdrop on a public message.
        let intercepted = router.eavesdrop_check(
            msg_id,
            eavesdropper,
            location,
            Decimal::ONE,
            0,
        );
        assert!(intercepted.is_none());
    }

    // -----------------------------------------------------------------------
    // 10. get_public_messages_at_location
    // -----------------------------------------------------------------------

    #[test]
    fn get_public_messages_filters_non_public() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let target = AgentId::new();
        let location = LocationId::new();

        let _ = router.send_message(make_public_message(sender, location, 10));
        let _ = router.send_message(make_whisper(sender, target, location, 10));
        let _ = router.send_message(make_announcement(sender, location, 10));

        let public_msgs = router.get_public_messages_at_location(location, 10);
        // Only the public message, not whisper or announcement.
        assert_eq!(public_msgs.len(), 1);
    }

    // -----------------------------------------------------------------------
    // 11. Default / empty router
    // -----------------------------------------------------------------------

    #[test]
    fn new_router_is_empty() {
        let router = MessageRouter::new();
        assert_eq!(router.message_count(), 0);
        let stats = router.message_count_by_visibility();
        assert_eq!(stats.total_messages, 0);
        assert_eq!(stats.public_count, 0);
        assert_eq!(stats.whisper_count, 0);
        assert_eq!(stats.conspire_count, 0);
        assert_eq!(stats.eavesdropped_count, 0);
    }

    // -----------------------------------------------------------------------
    // 12. compute_threshold_per_10000 helper
    // -----------------------------------------------------------------------

    #[test]
    fn threshold_computation_standard_values() {
        // 0.05 * 10000 = 500
        assert_eq!(compute_threshold_per_10000(Decimal::new(5, 2)), 500);
        // 0.15 * 10000 = 1500
        assert_eq!(compute_threshold_per_10000(Decimal::new(15, 2)), 1500);
        // 0.10 * 10000 = 1000
        assert_eq!(compute_threshold_per_10000(Decimal::new(10, 2)), 1000);
        // 0.0 * 10000 = 0
        assert_eq!(compute_threshold_per_10000(Decimal::ZERO), 0);
    }

    // -----------------------------------------------------------------------
    // 13. Multiple message types in one query
    // -----------------------------------------------------------------------

    #[test]
    fn agent_receives_all_applicable_message_types() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let receiver = AgentId::new();
        let location = LocationId::new();

        // Public message at receiver's location and tick.
        let _ = router.send_message(make_public_message(sender, location, 10));

        // Whisper to receiver.
        let _ = router.send_message(make_whisper(sender, receiver, location, 10));

        // Conspire including receiver.
        let _ = router.send_message(make_conspire(sender, vec![sender, receiver], location, 10));

        // Location announcement at receiver's location.
        let _ = router.send_message(make_announcement(sender, location, 5));

        let msgs = router.get_messages_for_agent(receiver, location, 10);
        // Should get: 1 public + 1 whisper + 1 conspire + 1 announcement = 4.
        assert_eq!(msgs.len(), 4);
    }

    // -----------------------------------------------------------------------
    // 14. Nonexistent message eavesdrop
    // -----------------------------------------------------------------------

    #[test]
    fn eavesdrop_nonexistent_message_returns_none() {
        let mut router = MessageRouter::new();
        let fake_id = Uuid::now_v7();
        let eavesdropper = AgentId::new();
        let location = LocationId::new();

        let intercepted = router.eavesdrop_check(
            fake_id,
            eavesdropper,
            location,
            Decimal::ONE,
            0,
        );
        assert!(intercepted.is_none());
    }

    // -----------------------------------------------------------------------
    // 15. Clear old messages boundary conditions
    // -----------------------------------------------------------------------

    #[test]
    fn clear_old_messages_at_exact_cutoff_tick_preserved() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let location = LocationId::new();

        // Message at tick 10.
        let _ = router.send_message(make_public_message(sender, location, 10));

        // current_tick = 15, retention = 5, cutoff = 10. Tick 10 >= 10, kept.
        router.clear_old_messages(15, 5);
        assert_eq!(router.message_count(), 1);
    }

    #[test]
    fn clear_old_messages_just_below_cutoff_removed() {
        let mut router = MessageRouter::new();
        let sender = AgentId::new();
        let location = LocationId::new();

        // Message at tick 9.
        let _ = router.send_message(make_public_message(sender, location, 9));

        // current_tick = 15, retention = 5, cutoff = 10. Tick 9 < 10, removed.
        router.clear_old_messages(15, 5);
        assert_eq!(router.message_count(), 0);
    }
}
