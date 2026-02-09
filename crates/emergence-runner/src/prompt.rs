//! Prompt template loading and rendering via `minijinja`.
//!
//! Templates are loaded from the filesystem (default: `templates/` directory)
//! so operators can tune agent behavior without recompiling. The template
//! engine renders perception data into a structured LLM prompt following
//! `agent-system.md` section 6.2.

use minijinja::Environment;

use crate::error::RunnerError;

/// Manages prompt template loading and rendering.
///
/// Wraps a `minijinja` [`Environment`] with all agent prompt templates
/// pre-loaded. Templates can be edited on disk and will be picked up on
/// the next call to [`PromptEngine::new`].
pub struct PromptEngine {
    env: Environment<'static>,
}

/// The complete rendered prompt ready to send to an LLM backend.
#[derive(Debug, Clone)]
pub struct RenderedPrompt {
    /// System message establishing the agent's reality.
    pub system: String,
    /// User message containing identity, perception, memory, and actions.
    pub user: String,
}

impl PromptEngine {
    /// Create a new prompt engine loading templates from the given directory.
    ///
    /// The directory must contain: `system.j2`, `identity.j2`,
    /// `perception.j2`, `memory.j2`, `actions.j2`.
    pub fn new(templates_dir: &str) -> Result<Self, RunnerError> {
        let mut env = Environment::new();

        let system_tpl = load_template(templates_dir, "system.j2")?;
        let identity_tpl = load_template(templates_dir, "identity.j2")?;
        let perception_tpl = load_template(templates_dir, "perception.j2")?;
        let memory_tpl = load_template(templates_dir, "memory.j2")?;
        let actions_tpl = load_template(templates_dir, "actions.j2")?;

        env.add_template_owned("system", system_tpl)
            .map_err(|e| RunnerError::Template(format!("failed to add system template: {e}")))?;
        env.add_template_owned("identity", identity_tpl)
            .map_err(|e| RunnerError::Template(format!("failed to add identity template: {e}")))?;
        env.add_template_owned("perception", perception_tpl).map_err(|e| {
            RunnerError::Template(format!("failed to add perception template: {e}"))
        })?;
        env.add_template_owned("memory", memory_tpl)
            .map_err(|e| RunnerError::Template(format!("failed to add memory template: {e}")))?;
        env.add_template_owned("actions", actions_tpl)
            .map_err(|e| RunnerError::Template(format!("failed to add actions template: {e}")))?;

        Ok(Self { env })
    }

    /// Render the full prompt for an agent's decision.
    ///
    /// Takes the perception data serialized as a `serde_json::Value` and
    /// produces a [`RenderedPrompt`] with system and user messages.
    pub fn render(
        &self,
        perception: &serde_json::Value,
    ) -> Result<RenderedPrompt, RunnerError> {
        let system = self
            .env
            .get_template("system")
            .map_err(|e| RunnerError::Template(format!("missing system template: {e}")))?
            .render(perception)
            .map_err(|e| RunnerError::Template(format!("system render failed: {e}")))?;

        let identity = self
            .env
            .get_template("identity")
            .map_err(|e| RunnerError::Template(format!("missing identity template: {e}")))?
            .render(perception)
            .map_err(|e| RunnerError::Template(format!("identity render failed: {e}")))?;

        let perception_text = self
            .env
            .get_template("perception")
            .map_err(|e| RunnerError::Template(format!("missing perception template: {e}")))?
            .render(perception)
            .map_err(|e| RunnerError::Template(format!("perception render failed: {e}")))?;

        let memory = self
            .env
            .get_template("memory")
            .map_err(|e| RunnerError::Template(format!("missing memory template: {e}")))?
            .render(perception)
            .map_err(|e| RunnerError::Template(format!("memory render failed: {e}")))?;

        let actions = self
            .env
            .get_template("actions")
            .map_err(|e| RunnerError::Template(format!("missing actions template: {e}")))?
            .render(perception)
            .map_err(|e| RunnerError::Template(format!("actions render failed: {e}")))?;

        let user = format!("{identity}\n\n{perception_text}\n\n{memory}\n\n{actions}");

        Ok(RenderedPrompt { system, user })
    }
}

/// Read a template file from disk.
fn load_template(dir: &str, filename: &str) -> Result<String, RunnerError> {
    let path = format!("{dir}/{filename}");
    std::fs::read_to_string(&path)
        .map_err(|e| RunnerError::Template(format!("failed to read {path}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_test_templates(dir: &std::path::Path) {
        std::fs::write(
            dir.join("system.j2"),
            "You are an agent named {{ self_state.name }} in a simulated world.",
        )
        .ok();
        std::fs::write(
            dir.join("identity.j2"),
            "## Identity\nName: {{ self_state.name }}\nAge: {{ self_state.age }}",
        )
        .ok();
        std::fs::write(
            dir.join("perception.j2"),
            "## Perception\nTick: {{ tick }}\nSeason: {{ season }}\nWeather: {{ weather }}\nLocation: {{ self_state.location_name }}",
        )
        .ok();
        std::fs::write(
            dir.join("memory.j2"),
            "## Memory\n{% for m in recent_memory %}- {{ m }}\n{% endfor %}",
        )
        .ok();
        std::fs::write(
            dir.join("actions.j2"),
            "## Available Actions\n{% for a in available_actions %}- {{ a }}\n{% endfor %}\n\nRespond with JSON: {\"action_type\": \"...\", \"parameters\": {...}}",
        )
        .ok();
    }

    #[test]
    fn template_loading_and_rendering() {
        let unique = format!(
            "emergence_test_templates_{}_{:?}",
            std::process::id(),
            std::thread::current().id(),
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).ok();
        write_test_templates(&dir);

        let engine =
            PromptEngine::new(dir.to_str().unwrap_or(""));
        assert!(engine.is_ok(), "PromptEngine::new should succeed with valid templates");

        let engine = match engine {
            Ok(e) => e,
            Err(_) => return,
        };

        let perception = serde_json::json!({
            "tick": 42,
            "time_of_day": "Morning",
            "season": "Summer",
            "weather": "Clear",
            "self_state": {
                "id": "01945c2a-3b4f-7def-8a12-bc34567890ab",
                "name": "Luna",
                "age": 15,
                "energy": 80,
                "health": 100,
                "hunger": 20,
                "location_name": "Riverside Camp",
                "inventory": {},
                "carry_load": "0/50",
                "active_goals": ["find food"],
                "known_skills": ["gathering (lvl 1)"]
            },
            "surroundings": {
                "location_description": "A camp by the river",
                "visible_resources": {"Wood": "abundant", "Water": "plentiful"},
                "structures_here": [],
                "agents_here": [],
                "messages_here": []
            },
            "known_routes": [],
            "recent_memory": ["Found berries nearby last tick"],
            "available_actions": ["gather", "eat", "rest", "move"],
            "notifications": []
        });

        let result = engine.render(&perception);
        assert!(result.is_ok(), "render should succeed with valid perception data");

        let prompt = match result {
            Ok(p) => p,
            Err(_) => return,
        };

        assert!(
            prompt.system.contains("Luna"),
            "system prompt should contain agent name"
        );
        assert!(
            prompt.user.contains("Tick: 42"),
            "user prompt should contain tick number"
        );
        assert!(
            prompt.user.contains("gather"),
            "user prompt should list available actions"
        );
        assert!(
            prompt.user.contains("Found berries"),
            "user prompt should contain memories"
        );

        // Cleanup
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_template_returns_error() {
        let unique = format!(
            "emergence_missing_templates_{}_{:?}",
            std::process::id(),
            std::thread::current().id(),
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).ok();
        // Write only some templates, leaving others missing
        std::fs::write(dir.join("system.j2"), "test").ok();

        let result = PromptEngine::new(dir.to_str().unwrap_or(""));
        assert!(result.is_err(), "should fail when templates are missing");

        std::fs::remove_dir_all(&dir).ok();
    }
}
