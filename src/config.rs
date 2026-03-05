use crate::commands::{CommandRegistry, CommandSpec};
use std::collections::{BTreeMap, BTreeSet};

/// Plugin configuration containing all settings and mappings
#[derive(Debug, Clone)]
pub struct PluginConfig {
    pub default_layout: Option<String>,
    pub feature_to_bit: BTreeMap<String, u8>,
    pub state_bits: BTreeMap<String, u64>,
    pub bits_to_state: BTreeMap<u64, String>,
    pub commands: CommandRegistry,
}

/// Trait for resolving features and states to bit patterns
pub trait BitResolver {
    fn bit_for_feature(&self, feature: &str) -> Option<u64>;
    fn bits_for_state(&self, state: &str) -> Option<u64>;
    fn default_bits(&self) -> Option<u64>;
}

impl BitResolver for PluginConfig {
    fn bit_for_feature(&self, feature: &str) -> Option<u64> {
        self.feature_to_bit.get(feature).map(|idx| 1u64 << idx)
    }

    fn bits_for_state(&self, state: &str) -> Option<u64> {
        self.state_bits.get(state).copied()
    }

    fn default_bits(&self) -> Option<u64> {
        self.default_layout
            .as_ref()
            .and_then(|name| self.state_bits.get(name))
            .copied()
    }
}

/// Trait for state resolution
pub trait StateResolver {
    fn resolve_target_bits(
        &self,
        current_bits: u64,
        command: &crate::commands::CommandSpec,
    ) -> Option<u64>;
    fn closest_state(&self, target_bits: u64) -> Option<(String, u64)>;
}

impl StateResolver for PluginConfig {
    fn resolve_target_bits(
        &self,
        current_bits: u64,
        command: &crate::commands::CommandSpec,
    ) -> Option<u64> {
        use crate::commands::{CommandKind, CommandTarget};

        match (&command.kind, &command.target) {
            (CommandKind::Toggle, CommandTarget::Feature(feature)) => {
                self.bit_for_feature(feature).map(|bit| current_bits ^ bit)
            }
            (CommandKind::Show, CommandTarget::Feature(feature)) => {
                self.bit_for_feature(feature).map(|bit| current_bits | bit)
            }
            (CommandKind::Hide, CommandTarget::Feature(feature)) => {
                self.bit_for_feature(feature).map(|bit| current_bits & !bit)
            }
            (CommandKind::SetState, CommandTarget::State(state)) => self.bits_for_state(state),
            (CommandKind::SetState, CommandTarget::Feature(state)) => self.bits_for_state(state),
            (_, CommandTarget::State(state)) => self.bits_for_state(state),
        }
    }

    fn closest_state(&self, target_bits: u64) -> Option<(String, u64)> {
        self.bits_to_state
            .iter()
            .map(|(bits, name)| ((bits ^ target_bits).count_ones(), name.clone(), *bits))
            .min_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)))
            .map(|(_, name, bits)| (name, bits))
    }
}

impl PluginConfig {
    /// Parse configuration from raw key-value pairs
    pub fn parse(raw: &BTreeMap<String, String>) -> Result<Self, String> {
        let mut default_layout = None;
        let mut layout_defs: Vec<(String, BTreeMap<String, bool>)> = Vec::new();
        let mut commands = BTreeMap::new();

        for (key, value) in raw {
            if key == "default_layout" {
                default_layout = Some(value.trim().to_string());
                continue;
            }

            if let Some(name) = key.strip_prefix("layout.") {
                let layout_features = parse_layout_line(value)?;
                layout_defs.push((name.to_string(), layout_features));
                continue;
            }

            if let Some(name) = key.strip_prefix("trigger.") {
                let command = CommandSpec::parse(value)?;
                commands.insert(name.to_string(), command);
                continue;
            }
        }

        if layout_defs.is_empty() {
            return Err("no layouts configured".into());
        }

        let mut feature_set = BTreeSet::new();
        for (_, feature_map) in &layout_defs {
            for feature in feature_map.keys() {
                feature_set.insert(feature.clone());
            }
        }

        if feature_set.is_empty() {
            return Err("no features declared in layouts".into());
        }

        if feature_set.len() > 64 {
            return Err("supports up to 64 features".into());
        }

        let feature_order: Vec<String> = feature_set.into_iter().collect();
        let feature_to_bit = feature_order
            .iter()
            .enumerate()
            .map(|(idx, name)| (name.clone(), idx as u8))
            .collect::<BTreeMap<_, _>>();

        let mut state_bits = BTreeMap::new();
        let mut bits_to_state = BTreeMap::new();
        for (name, layout_features) in layout_defs {
            let mut bits = 0u64;
            for feature_name in feature_order.iter() {
                let enabled = layout_features
                    .get(feature_name)
                    .ok_or_else(|| format!("layout '{name}' missing explicit value for feature '{feature_name}' - all features must be explicitly set in all layouts"))?;
                if *enabled {
                    bits |= 1u64 << feature_to_bit[feature_name];
                }
            }

            if let Some(existing) = bits_to_state.get(&bits) {
                return Err(format!(
                    "duplicate bitmask {bits} for layouts '{existing}' and '{name}'"
                ));
            }

            bits_to_state.insert(bits, name.clone());
            state_bits.insert(name, bits);
        }

        let default_layout = default_layout.or_else(|| state_bits.keys().next().cloned());

        Ok(Self {
            default_layout,
            feature_to_bit,
            state_bits,
            bits_to_state,
            commands: CommandRegistry::new(commands),
        })
    }
}

/// Parse a layout definition line into a feature map
fn parse_layout_line(raw: &str) -> Result<BTreeMap<String, bool>, String> {
    let mut features = BTreeMap::new();

    if raw.trim().is_empty() {
        return Err("empty layout definition".into());
    }

    for chunk in raw.split(',') {
        let part = chunk.trim();
        if part.is_empty() {
            continue;
        }

        let mut pieces = part.splitn(2, '=');
        let feature = pieces
            .next()
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .ok_or_else(|| "missing feature name".to_string())?;

        let value = pieces.next().map(str::trim).unwrap_or("true");
        let enabled = match value {
            "true" | "1" | "on" => true,
            "false" | "0" | "off" => false,
            _ => {
                return Err(format!(
                    "invalid feature value '{value}' (expected true/false)"
                ))
            }
        };

        features.insert(feature.to_string(), enabled);
    }

    Ok(features)
}
