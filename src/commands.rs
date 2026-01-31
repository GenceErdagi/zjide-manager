use std::collections::BTreeMap;

/// Represents the type of command to execute
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    Toggle,
    Show,
    Hide,
    SetState,
}

/// Represents the target of a command
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandTarget {
    Feature(String),
    State(String),
}

/// Represents a parsed command specification
#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub kind: CommandKind,
    pub target: CommandTarget,
}

impl CommandSpec {
    /// Parse a command specification from a raw string
    pub fn parse(raw: &str) -> Result<Self, String> {
        let normalized = raw.replace([':', '='], " ");
        let tokens: Vec<&str> = normalized
            .split_whitespace()
            .filter(|token| !token.is_empty())
            .collect();
        if tokens.is_empty() {
            return Err("empty trigger definition".into());
        }

        let head = tokens[0].to_ascii_lowercase();
        let tail = tokens.get(1).copied();

        let (kind, target) = match head.as_str() {
            "toggle" => (
                CommandKind::Toggle,
                CommandTarget::Feature(
                    tail.ok_or_else(|| "toggle command missing feature".to_string())?
                        .to_string(),
                ),
            ),
            "show" | "on" => (
                CommandKind::Show,
                CommandTarget::Feature(
                    tail.ok_or_else(|| "show command missing feature".to_string())?
                        .to_string(),
                ),
            ),
            "hide" | "off" => (
                CommandKind::Hide,
                CommandTarget::Feature(
                    tail.ok_or_else(|| "hide command missing feature".to_string())?
                        .to_string(),
                ),
            ),
            "state" | "set_state" | "layout" => (
                CommandKind::SetState,
                CommandTarget::State(
                    tail.ok_or_else(|| "set_state command missing layout name".to_string())?
                        .to_string(),
                ),
            ),
            _ => (
                CommandKind::Toggle,
                CommandTarget::Feature(tokens[0].to_string()),
            ),
        };

        Ok(Self { kind, target })
    }
}

/// Manages all available commands
#[derive(Debug, Clone)]
pub struct CommandRegistry {
    commands: BTreeMap<String, CommandSpec>,
}

impl CommandRegistry {
    pub fn new(commands: BTreeMap<String, CommandSpec>) -> Self {
        Self { commands }
    }

    pub fn get(&self, name: &str) -> Option<&CommandSpec> {
        self.commands.get(name)
    }

    pub fn insert(&mut self, name: String, command: CommandSpec) {
        self.commands.insert(name, command);
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self {
            commands: BTreeMap::new(),
        }
    }
}
