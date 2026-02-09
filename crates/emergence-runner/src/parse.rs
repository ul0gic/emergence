//! LLM response parsing into typed action requests.
//!
//! The LLM returns raw text (ideally JSON). This module extracts and
//! validates the response into an [`ActionParameters`] from `emergence-types`.
//! Malformed responses are handled gracefully by returning `NoAction`.

use emergence_types::{ActionParameters, ActionType};
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
pub fn parse_llm_response(raw: &str) -> ParsedDecision {
    match try_parse(raw) {
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
fn try_parse(raw: &str) -> Result<ParsedDecision, RunnerError> {
    let trimmed = raw.trim();

    // Strategy 1: direct parse
    if let Ok(parsed) = serde_json::from_str::<RawLlmResponse>(trimmed) {
        return convert_raw_response(parsed);
    }

    // Strategy 2: extract from markdown code block
    if let Some(json_str) = extract_json_from_codeblock(trimmed)
        && let Ok(parsed) = serde_json::from_str::<RawLlmResponse>(json_str)
    {
        return convert_raw_response(parsed);
    }

    // Strategy 3: strip trailing commas and retry
    let cleaned = strip_trailing_commas(trimmed);
    if let Ok(parsed) = serde_json::from_str::<RawLlmResponse>(&cleaned) {
        return convert_raw_response(parsed);
    }

    // Strategy 4: extract from code block then strip commas
    if let Some(json_str) = extract_json_from_codeblock(trimmed) {
        let cleaned_inner = strip_trailing_commas(json_str);
        if let Ok(parsed) = serde_json::from_str::<RawLlmResponse>(&cleaned_inner) {
            return convert_raw_response(parsed);
        }
    }

    Err(RunnerError::Parse(format!(
        "all parse strategies failed for: {trimmed}"
    )))
}

/// Convert a deserialized raw response into a typed decision.
fn convert_raw_response(raw: RawLlmResponse) -> Result<ParsedDecision, RunnerError> {
    let action_type = parse_action_type(&raw.action_type)?;
    let parameters = build_parameters(action_type, &raw.parameters)?;

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
            let uuid = uuid::Uuid::parse_str(dest_str)
                .map_err(|e| RunnerError::Parse(format!("invalid destination UUID: {e}")))?;
            Ok(ActionParameters::Move {
                destination: emergence_types::LocationId::from(uuid),
            })
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
            let uuid = uuid::Uuid::parse_str(target_str)
                .map_err(|e| RunnerError::Parse(format!("invalid target_agent UUID: {e}")))?;
            let message = params
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_owned();
            Ok(ActionParameters::Communicate {
                target_agent: emergence_types::AgentId::from(uuid),
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
        _ => {
            // Try constructing the variant name from the action type
            let variant_json = serde_json::json!({ format!("{action_type:?}"): params });
            serde_json::from_value::<ActionParameters>(variant_json).map_err(|e| {
                RunnerError::Parse(format!(
                    "failed to parse parameters for {action_type:?}: {e}"
                ))
            })
        }
    }
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

    #[test]
    fn parse_valid_gather() {
        let raw = r#"{"action_type": "Gather", "parameters": {"resource": "Wood"}, "reasoning": "I need wood"}"#;
        let decision = parse_llm_response(raw);
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
        let decision = parse_llm_response(raw);
        assert_eq!(decision.action_type, ActionType::Rest);
        assert!(matches!(decision.parameters, ActionParameters::Rest));
    }

    #[test]
    fn parse_noaction() {
        let raw = r#"{"action_type": "NoAction", "parameters": {}}"#;
        let decision = parse_llm_response(raw);
        assert_eq!(decision.action_type, ActionType::NoAction);
    }

    #[test]
    fn parse_case_insensitive() {
        let raw = r#"{"action_type": "gather", "parameters": {"resource": "Stone"}}"#;
        let decision = parse_llm_response(raw);
        assert_eq!(decision.action_type, ActionType::Gather);
    }

    #[test]
    fn parse_from_codeblock() {
        let raw = r#"Here is my decision:

```json
{"action_type": "Drink", "parameters": {}}
```

I chose to drink because I am thirsty."#;
        let decision = parse_llm_response(raw);
        assert_eq!(decision.action_type, ActionType::Drink);
    }

    #[test]
    fn parse_trailing_comma() {
        let raw = r#"{"action_type": "Rest", "parameters": {}, "reasoning": "tired",}"#;
        let decision = parse_llm_response(raw);
        assert_eq!(decision.action_type, ActionType::Rest);
    }

    #[test]
    fn parse_garbage_returns_noaction() {
        let raw = "I think I should gather some wood. Let me do that.";
        let decision = parse_llm_response(raw);
        assert_eq!(decision.action_type, ActionType::NoAction);
    }

    #[test]
    fn parse_empty_returns_noaction() {
        let decision = parse_llm_response("");
        assert_eq!(decision.action_type, ActionType::NoAction);
    }

    #[test]
    fn parse_with_goal_updates() {
        let raw = r#"{"action_type": "Gather", "parameters": {"resource": "Wood"}, "goal_update": ["build shelter", "explore north"]}"#;
        let decision = parse_llm_response(raw);
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
        let decision = parse_llm_response(raw);
        assert_eq!(decision.action_type, ActionType::NoAction);

        let raw2 = r#"{"action_type": "trade_offer", "parameters": {"target_agent": "01945c2a-3b4f-7def-8a12-bc34567890ab", "offer": {"Wood": 5}, "request": {"Stone": 3}}}"#;
        let decision2 = parse_llm_response(raw2);
        assert_eq!(decision2.action_type, ActionType::TradeOffer);
    }

    #[test]
    fn parse_broadcast() {
        let raw = r#"{"action_type": "Broadcast", "parameters": {"message": "Hello everyone!"}}"#;
        let decision = parse_llm_response(raw);
        assert_eq!(decision.action_type, ActionType::Broadcast);
        assert!(matches!(
            decision.parameters,
            ActionParameters::Broadcast { ref message } if message == "Hello everyone!"
        ));
    }
}
