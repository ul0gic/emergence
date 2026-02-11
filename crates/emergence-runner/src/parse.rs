//! LLM response parsing into typed action requests.
//!
//! The LLM returns raw text (ideally JSON). This module extracts and
//! validates the response into an [`ActionParameters`] from `emergence-types`.
//! Malformed responses are handled gracefully by returning `NoAction`.

use std::collections::BTreeMap;

use emergence_types::{ActionParameters, ActionType, AgentId, KnownRoute};
use tracing::warn;

use crate::error::RunnerError;

/// The parsed decision from an LLM response.
#[derive(Debug, Clone)]
pub struct ParsedDecision {
    /// The action type chosen by the agent.
    pub action_type: ActionType,
    /// The typed action parameters.
    pub parameters: ActionParameters,
    /// The agent's reasoning (logged for debugging, not used by the engine).
    pub reasoning: Option<String>,
    /// Goal updates the agent wants to make.
    ///
    /// Will be used by the reflection system in Phase 3 to update
    /// the agent's goal list in Dragonfly.
    #[allow(dead_code)]
    pub goal_updates: Vec<String>,
}

/// Intermediate struct for deserializing the LLM's raw JSON response.
///
/// The LLM produces a flat JSON object with `action_type` and `parameters`
/// at the top level. This struct captures that shape before we validate it
/// against the typed [`ActionParameters`] enum.
#[derive(Debug, serde::Deserialize)]
struct RawLlmResponse {
    action_type: String,
    #[serde(default)]
    parameters: serde_json::Value,
    #[serde(default)]
    reasoning: Option<String>,
    #[serde(default)]
    goal_update: Option<Vec<String>>,
}

/// Parse an LLM response string into a validated [`ParsedDecision`].
///
/// Attempts multiple recovery strategies if the raw text is not clean JSON:
/// 1. Direct `serde_json` deserialization
/// 2. Extract JSON from markdown code blocks
/// 3. Strip trailing commas and retry
///
/// If all attempts fail, returns [`ActionType::NoAction`] with a warning log.
///
/// When `known_routes` is provided, Move actions can resolve location names
/// to UUIDs (fallback for LLMs that send names instead of IDs).
///
/// When `agent_name_map` is provided, actions with `target_agent` fields
/// can resolve agent names to UUIDs (fallback for LLMs that send names
/// like "Iris" instead of agent UUIDs).
pub fn parse_llm_response(
    raw: &str,
    known_routes: &[KnownRoute],
    agent_name_map: &BTreeMap<String, AgentId>,
) -> ParsedDecision {
    match try_parse(raw, known_routes, agent_name_map) {
        Ok(decision) => decision,
        Err(e) => {
            warn!(
                error = %e,
                raw_response = raw,
                "failed to parse LLM response, returning NoAction"
            );
            no_action_decision()
        }
    }
}

/// Attempt to parse the response through multiple recovery strategies.
fn try_parse(
    raw: &str,
    known_routes: &[KnownRoute],
    agent_name_map: &BTreeMap<String, AgentId>,
) -> Result<ParsedDecision, RunnerError> {
    let trimmed = raw.trim();

    // Strategy 1: direct parse
    if let Ok(parsed) = serde_json::from_str::<RawLlmResponse>(trimmed) {
        return convert_raw_response(parsed, known_routes, agent_name_map);
    }

    // Strategy 2: extract from markdown code block
    if let Some(json_str) = extract_json_from_codeblock(trimmed)
        && let Ok(parsed) = serde_json::from_str::<RawLlmResponse>(json_str)
    {
        return convert_raw_response(parsed, known_routes, agent_name_map);
    }

    // Strategy 3: strip trailing commas and retry
    let cleaned = strip_trailing_commas(trimmed);
    if let Ok(parsed) = serde_json::from_str::<RawLlmResponse>(&cleaned) {
        return convert_raw_response(parsed, known_routes, agent_name_map);
    }

    // Strategy 4: extract from code block then strip commas
    if let Some(json_str) = extract_json_from_codeblock(trimmed) {
        let cleaned_inner = strip_trailing_commas(json_str);
        if let Ok(parsed) = serde_json::from_str::<RawLlmResponse>(&cleaned_inner) {
            return convert_raw_response(parsed, known_routes, agent_name_map);
        }
    }

    Err(RunnerError::Parse(format!(
        "all parse strategies failed for: {trimmed}"
    )))
}

/// Convert a deserialized raw response into a typed decision.
fn convert_raw_response(
    raw: RawLlmResponse,
    known_routes: &[KnownRoute],
    agent_name_map: &BTreeMap<String, AgentId>,
) -> Result<ParsedDecision, RunnerError> {
    let action_type = parse_action_type(&raw.action_type)?;
    let parameters = build_parameters(action_type, &raw.parameters, known_routes, agent_name_map)?;

    Ok(ParsedDecision {
        action_type,
        parameters,
        reasoning: raw.reasoning,
        goal_updates: raw.goal_update.unwrap_or_default(),
    })
}

/// Parse a string action type into the typed enum.
fn parse_action_type(s: &str) -> Result<ActionType, RunnerError> {
    // Try serde deserialization first (handles exact enum variant names)
    let quoted = format!("\"{s}\"");
    if let Ok(at) = serde_json::from_str::<ActionType>(&quoted) {
        return Ok(at);
    }

    // Fallback: case-insensitive matching for common LLM outputs
    match s.to_lowercase().as_str() {
        "gather" => Ok(ActionType::Gather),
        "eat" => Ok(ActionType::Eat),
        "drink" => Ok(ActionType::Drink),
        "rest" => Ok(ActionType::Rest),
        "move" => Ok(ActionType::Move),
        "build" => Ok(ActionType::Build),
        "repair" => Ok(ActionType::Repair),
        "demolish" => Ok(ActionType::Demolish),
        "improveroute" | "improve_route" => Ok(ActionType::ImproveRoute),
        "communicate" => Ok(ActionType::Communicate),
        "broadcast" => Ok(ActionType::Broadcast),
        "tradeoffer" | "trade_offer" => Ok(ActionType::TradeOffer),
        "tradeaccept" | "trade_accept" => Ok(ActionType::TradeAccept),
        "tradereject" | "trade_reject" => Ok(ActionType::TradeReject),
        "formgroup" | "form_group" => Ok(ActionType::FormGroup),
        "teach" => Ok(ActionType::Teach),
        "farmplant" | "farm_plant" => Ok(ActionType::FarmPlant),
        "farmharvest" | "farm_harvest" => Ok(ActionType::FarmHarvest),
        "craft" => Ok(ActionType::Craft),
        "mine" => Ok(ActionType::Mine),
        "smelt" => Ok(ActionType::Smelt),
        "write" => Ok(ActionType::Write),
        "read" => Ok(ActionType::Read),
        "claim" => Ok(ActionType::Claim),
        "legislate" => Ok(ActionType::Legislate),
        "enforce" => Ok(ActionType::Enforce),
        "reproduce" => Ok(ActionType::Reproduce),
        "noaction" | "no_action" | "none" => Ok(ActionType::NoAction),
        other => Err(RunnerError::Parse(format!("unknown action type: {other}"))),
    }
}

/// Build typed [`ActionParameters`] from the action type and raw JSON params.
fn build_parameters(
    action_type: ActionType,
    params: &serde_json::Value,
    known_routes: &[KnownRoute],
    agent_name_map: &BTreeMap<String, AgentId>,
) -> Result<ActionParameters, RunnerError> {
    match action_type {
        ActionType::Gather => {
            let resource = params
                .get("resource")
                .ok_or_else(|| RunnerError::Parse("Gather requires 'resource' parameter".to_owned()))?;
            let resource_str = format!("\"{resource_value}\"", resource_value = resource.as_str().unwrap_or("Wood"));
            let resource: emergence_types::Resource = serde_json::from_str(&resource_str)
                .map_err(|e| RunnerError::Parse(format!("invalid resource: {e}")))?;
            Ok(ActionParameters::Gather { resource })
        }
        ActionType::Eat => {
            let food = params
                .get("food_type")
                .ok_or_else(|| RunnerError::Parse("Eat requires 'food_type' parameter".to_owned()))?;
            let food_str = format!("\"{val}\"", val = food.as_str().unwrap_or("FoodBerry"));
            let food_type: emergence_types::Resource = serde_json::from_str(&food_str)
                .map_err(|e| RunnerError::Parse(format!("invalid food_type: {e}")))?;
            Ok(ActionParameters::Eat { food_type })
        }
        ActionType::Drink => Ok(ActionParameters::Drink),
        ActionType::Rest => Ok(ActionParameters::Rest),
        ActionType::Move => {
            let dest = params
                .get("destination")
                .ok_or_else(|| RunnerError::Parse("Move requires 'destination' parameter".to_owned()))?;
            let dest_str = dest.as_str().unwrap_or("");

            // Try parsing as UUID first
            if let Ok(uuid) = uuid::Uuid::parse_str(dest_str) {
                return Ok(ActionParameters::Move {
                    destination: emergence_types::LocationId::from(uuid),
                });
            }

            // Fallback: resolve location name to UUID via known routes
            let name_lower = dest_str.to_lowercase();
            for route in known_routes {
                if route.destination.to_lowercase() == name_lower
                    && let Ok(uuid) = uuid::Uuid::parse_str(&route.destination_id)
                {
                    warn!(
                        name = dest_str,
                        resolved_id = %route.destination_id,
                        "Move destination resolved from name to UUID"
                    );
                    return Ok(ActionParameters::Move {
                        destination: emergence_types::LocationId::from(uuid),
                    });
                }
            }

            Err(RunnerError::Parse(format!(
                "Move destination '{dest_str}' is not a valid UUID and does not match any known route"
            )))
        }
        ActionType::Build => {
            let st = params
                .get("structure_type")
                .ok_or_else(|| RunnerError::Parse("Build requires 'structure_type' parameter".to_owned()))?;
            let st_str = format!("\"{val}\"", val = st.as_str().unwrap_or("LeanTo"));
            let structure_type: emergence_types::StructureType = serde_json::from_str(&st_str)
                .map_err(|e| RunnerError::Parse(format!("invalid structure_type: {e}")))?;
            Ok(ActionParameters::Build { structure_type })
        }
        ActionType::Communicate => {
            let target = params
                .get("target_agent")
                .ok_or_else(|| RunnerError::Parse("Communicate requires 'target_agent'".to_owned()))?;
            let target_str = target.as_str().unwrap_or("");
            let target_agent = resolve_target_agent(target_str, agent_name_map)?;
            let message = params
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_owned();
            Ok(ActionParameters::Communicate {
                target_agent,
                message,
            })
        }
        ActionType::Broadcast => {
            let message = params
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_owned();
            Ok(ActionParameters::Broadcast { message })
        }
        ActionType::NoAction => Ok(ActionParameters::NoAction),
        // For all other action types, attempt a direct serde deserialize
        // of the parameters into the matching ActionParameters variant.
        // If the params contain a `target_agent` that is a name rather
        // than a UUID, resolve it first so serde deserialization succeeds.
        _ => {
            let resolved_params = resolve_target_agent_in_params(params, agent_name_map);
            let variant_json = serde_json::json!({ format!("{action_type:?}"): resolved_params });
            serde_json::from_value::<ActionParameters>(variant_json).map_err(|e| {
                RunnerError::Parse(format!(
                    "failed to parse parameters for {action_type:?}: {e}"
                ))
            })
        }
    }
}

/// Resolve a `target_agent` string to an [`AgentId`].
///
/// Tries parsing as a UUID first. If that fails, performs a case-insensitive
/// lookup in the provided `agent_name_map` (built from the perception's
/// `agents_here` list). This handles the common case where the LLM sends
/// an agent name like "Iris" instead of the UUID.
fn resolve_target_agent(
    target_str: &str,
    agent_name_map: &BTreeMap<String, AgentId>,
) -> Result<AgentId, RunnerError> {
    // Try parsing as UUID first (the expected/fast path)
    if let Ok(uuid) = uuid::Uuid::parse_str(target_str) {
        return Ok(AgentId::from(uuid));
    }

    // Fallback: case-insensitive name lookup
    let name_lower = target_str.to_lowercase();
    for (name, &agent_id) in agent_name_map {
        if name.to_lowercase() == name_lower {
            warn!(
                name = target_str,
                resolved_id = %agent_id,
                "target_agent resolved from name to UUID"
            );
            return Ok(agent_id);
        }
    }

    Err(RunnerError::Parse(format!(
        "target_agent '{target_str}' is not a valid UUID and does not match any nearby agent"
    )))
}

/// If the params JSON object contains a `target_agent` field that is not a
/// valid UUID, attempt to resolve it via the agent name map and return
/// a new params object with the UUID substituted. If the field is already
/// a UUID or absent, returns the params unchanged.
fn resolve_target_agent_in_params(
    params: &serde_json::Value,
    agent_name_map: &BTreeMap<String, AgentId>,
) -> serde_json::Value {
    let Some(obj) = params.as_object() else {
        return params.clone();
    };

    let Some(target_val) = obj.get("target_agent") else {
        return params.clone();
    };

    let Some(target_str) = target_val.as_str() else {
        return params.clone();
    };

    // If it already parses as UUID, no rewriting needed
    if uuid::Uuid::parse_str(target_str).is_ok() {
        return params.clone();
    }

    // Try name resolution
    if let Ok(agent_id) = resolve_target_agent(target_str, agent_name_map) {
        let mut new_obj = obj.clone();
        new_obj.insert(
            "target_agent".to_owned(),
            serde_json::Value::String(agent_id.to_string()),
        );
        return serde_json::Value::Object(new_obj);
    }

    // Could not resolve; return original and let serde fail with a clear error
    params.clone()
}

/// Extract JSON from a markdown code block.
fn extract_json_from_codeblock(text: &str) -> Option<&str> {
    // Look for ```json ... ``` or ``` ... ```
    let start = text.find("```json").map(|i| {
        let after_tag = i.checked_add(7).unwrap_or(i);
        // Find the newline after ```json
        text.get(after_tag..)
            .and_then(|s| s.find('\n'))
            .and_then(|nl| after_tag.checked_add(nl))
            .and_then(|pos| pos.checked_add(1))
            .unwrap_or(after_tag)
    }).or_else(|| {
        text.find("```").map(|i| {
            let after_tag = i.checked_add(3).unwrap_or(i);
            text.get(after_tag..)
                .and_then(|s| s.find('\n'))
                .and_then(|nl| after_tag.checked_add(nl))
                .and_then(|pos| pos.checked_add(1))
                .unwrap_or(after_tag)
        })
    });

    let start = start?;
    let remaining = text.get(start..)?;
    let end = remaining.find("```")?;
    remaining.get(..end).map(str::trim)
}

/// Strip trailing commas before closing braces and brackets (common LLM error).
fn strip_trailing_commas(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();

    let mut i = 0;
    while i < len {
        let c = chars.get(i).copied().unwrap_or(' ');
        if c == ',' {
            // Look ahead past whitespace for } or ]
            let mut j = i.checked_add(1).unwrap_or(i);
            while j < len && chars.get(j).copied().unwrap_or(' ').is_whitespace() {
                j = j.checked_add(1).unwrap_or(j);
            }
            let next = chars.get(j).copied().unwrap_or(' ');
            if next == '}' || next == ']' {
                // Skip this comma
                i = i.checked_add(1).unwrap_or(i);
                continue;
            }
        }
        result.push(c);
        i = i.checked_add(1).unwrap_or(len);
    }

    result
}

/// Construct a default no-action decision.
fn no_action_decision() -> ParsedDecision {
    ParsedDecision {
        action_type: ActionType::NoAction,
        parameters: ActionParameters::NoAction,
        reasoning: Some("Failed to parse LLM response".to_owned()),
        goal_updates: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shorthand for an empty name map (most tests don't need agent name resolution).
    fn no_names() -> BTreeMap<String, AgentId> {
        BTreeMap::new()
    }

    #[test]
    fn parse_valid_gather() {
        let raw = r#"{"action_type": "Gather", "parameters": {"resource": "Wood"}, "reasoning": "I need wood"}"#;
        let decision = parse_llm_response(raw, &[], &no_names());
        assert_eq!(decision.action_type, ActionType::Gather);
        assert!(matches!(
            decision.parameters,
            ActionParameters::Gather { resource: emergence_types::Resource::Wood }
        ));
        assert_eq!(decision.reasoning.as_deref(), Some("I need wood"));
    }

    #[test]
    fn parse_valid_rest() {
        let raw = r#"{"action_type": "Rest", "parameters": {}, "reasoning": "I am tired"}"#;
        let decision = parse_llm_response(raw, &[], &no_names());
        assert_eq!(decision.action_type, ActionType::Rest);
        assert!(matches!(decision.parameters, ActionParameters::Rest));
    }

    #[test]
    fn parse_noaction() {
        let raw = r#"{"action_type": "NoAction", "parameters": {}}"#;
        let decision = parse_llm_response(raw, &[], &no_names());
        assert_eq!(decision.action_type, ActionType::NoAction);
    }

    #[test]
    fn parse_case_insensitive() {
        let raw = r#"{"action_type": "gather", "parameters": {"resource": "Stone"}}"#;
        let decision = parse_llm_response(raw, &[], &no_names());
        assert_eq!(decision.action_type, ActionType::Gather);
    }

    #[test]
    fn parse_from_codeblock() {
        let raw = r#"Here is my decision:

```json
{"action_type": "Drink", "parameters": {}}
```

I chose to drink because I am thirsty."#;
        let decision = parse_llm_response(raw, &[], &no_names());
        assert_eq!(decision.action_type, ActionType::Drink);
    }

    #[test]
    fn parse_trailing_comma() {
        let raw = r#"{"action_type": "Rest", "parameters": {}, "reasoning": "tired",}"#;
        let decision = parse_llm_response(raw, &[], &no_names());
        assert_eq!(decision.action_type, ActionType::Rest);
    }

    #[test]
    fn parse_garbage_returns_noaction() {
        let raw = "I think I should gather some wood. Let me do that.";
        let decision = parse_llm_response(raw, &[], &no_names());
        assert_eq!(decision.action_type, ActionType::NoAction);
    }

    #[test]
    fn parse_empty_returns_noaction() {
        let decision = parse_llm_response("", &[], &no_names());
        assert_eq!(decision.action_type, ActionType::NoAction);
    }

    #[test]
    fn parse_with_goal_updates() {
        let raw = r#"{"action_type": "Gather", "parameters": {"resource": "Wood"}, "goal_update": ["build shelter", "explore north"]}"#;
        let decision = parse_llm_response(raw, &[], &no_names());
        assert_eq!(decision.action_type, ActionType::Gather);
        assert_eq!(decision.goal_updates.len(), 2);
    }

    #[test]
    fn extract_json_from_markdown() {
        let text = "```json\n{\"key\": \"value\"}\n```";
        let result = extract_json_from_codeblock(text);
        assert_eq!(result, Some("{\"key\": \"value\"}"));
    }

    #[test]
    fn extract_json_from_plain_codeblock() {
        let text = "```\n{\"key\": \"value\"}\n```";
        let result = extract_json_from_codeblock(text);
        assert_eq!(result, Some("{\"key\": \"value\"}"));
    }

    #[test]
    fn strip_trailing_commas_basic() {
        let input = r#"{"a": 1, "b": 2,}"#;
        let result = strip_trailing_commas(input);
        assert_eq!(result, r#"{"a": 1, "b": 2}"#);
    }

    #[test]
    fn strip_trailing_commas_array() {
        let input = r#"[1, 2, 3,]"#;
        let result = strip_trailing_commas(input);
        assert_eq!(result, "[1, 2, 3]");
    }

    #[test]
    fn parse_snake_case_action_types() {
        let raw = r#"{"action_type": "no_action", "parameters": {}}"#;
        let decision = parse_llm_response(raw, &[], &no_names());
        assert_eq!(decision.action_type, ActionType::NoAction);

        let raw2 = r#"{"action_type": "trade_offer", "parameters": {"target_agent": "01945c2a-3b4f-7def-8a12-bc34567890ab", "offer": {"Wood": 5}, "request": {"Stone": 3}}}"#;
        let decision2 = parse_llm_response(raw2, &[], &no_names());
        assert_eq!(decision2.action_type, ActionType::TradeOffer);
    }

    #[test]
    fn parse_broadcast() {
        let raw = r#"{"action_type": "Broadcast", "parameters": {"message": "Hello everyone!"}}"#;
        let decision = parse_llm_response(raw, &[], &no_names());
        assert_eq!(decision.action_type, ActionType::Broadcast);
        assert!(matches!(
            decision.parameters,
            ActionParameters::Broadcast { ref message } if message == "Hello everyone!"
        ));
    }

    // -----------------------------------------------------------------------
    // Agent name -> UUID resolution tests
    // -----------------------------------------------------------------------

    #[test]
    fn communicate_with_uuid_still_works() {
        let target_id = AgentId::new();
        let raw = format!(
            r#"{{"action_type": "Communicate", "parameters": {{"target_agent": "{target_id}", "message": "Hello"}}}}"#
        );
        let decision = parse_llm_response(&raw, &[], &no_names());
        assert_eq!(decision.action_type, ActionType::Communicate);
        assert!(matches!(
            decision.parameters,
            ActionParameters::Communicate { target_agent, ref message }
            if target_agent == target_id && message == "Hello"
        ));
    }

    #[test]
    fn communicate_with_name_resolves_to_uuid() {
        let iris_id = AgentId::new();
        let mut names = BTreeMap::new();
        names.insert("Iris".to_owned(), iris_id);

        let raw = r#"{"action_type": "Communicate", "parameters": {"target_agent": "Iris", "message": "Hello, would you like to gather together?"}}"#;
        let decision = parse_llm_response(raw, &[], &names);
        assert_eq!(decision.action_type, ActionType::Communicate);
        assert!(matches!(
            decision.parameters,
            ActionParameters::Communicate { target_agent, ref message }
            if target_agent == iris_id && message == "Hello, would you like to gather together?"
        ));
    }

    #[test]
    fn communicate_with_name_case_insensitive() {
        let clay_id = AgentId::new();
        let mut names = BTreeMap::new();
        names.insert("Clay".to_owned(), clay_id);

        let raw = r#"{"action_type": "Communicate", "parameters": {"target_agent": "clay", "message": "Hi"}}"#;
        let decision = parse_llm_response(raw, &[], &names);
        assert_eq!(decision.action_type, ActionType::Communicate);
        assert!(matches!(
            decision.parameters,
            ActionParameters::Communicate { target_agent, .. }
            if target_agent == clay_id
        ));
    }

    #[test]
    fn communicate_with_unknown_name_returns_noaction() {
        let raw = r#"{"action_type": "Communicate", "parameters": {"target_agent": "Nobody", "message": "Hi"}}"#;
        let decision = parse_llm_response(raw, &[], &no_names());
        assert_eq!(decision.action_type, ActionType::NoAction);
    }

    #[test]
    fn resolve_target_agent_uuid_path() {
        let id = AgentId::new();
        let result = resolve_target_agent(&id.to_string(), &no_names());
        assert!(result.is_ok());
        assert_eq!(result.unwrap_or(AgentId::new()), id);
    }

    #[test]
    fn resolve_target_agent_name_path() {
        let juniper_id = AgentId::new();
        let mut names = BTreeMap::new();
        names.insert("Juniper".to_owned(), juniper_id);

        let result = resolve_target_agent("Juniper", &names);
        assert!(result.is_ok());
        assert_eq!(result.unwrap_or(AgentId::new()), juniper_id);
    }

    #[test]
    fn resolve_target_agent_unknown_name_fails() {
        let result = resolve_target_agent("Ghost", &no_names());
        assert!(result.is_err());
    }
}
