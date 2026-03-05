use crate::config::{BitResolver, PluginConfig, StateResolver};
use std::collections::BTreeMap;
use zellij_tile::prelude::{go_to_swap_layout, TabInfo};

/// Represents the state of a single tab
#[derive(Default, Debug, Clone)]
pub struct TabState {
    pub active_layout: Option<String>,
    pub last_bits: Option<u64>,
    pub is_dirty: bool,
}

/// Manages the overall plugin state and tab tracking
pub struct State {
    pub config: Option<PluginConfig>,
    pub tabs: BTreeMap<usize, TabState>,
    pub active_tab: Option<usize>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            config: None,
            tabs: BTreeMap::new(),
            active_tab: None,
        }
    }
}

/// Trait for managing tab updates
pub trait TabManager {
    fn update_tabs(&mut self, tabs: Vec<TabInfo>);
    fn get_active_tab_state(&self) -> Option<&TabState>;
}

impl TabManager for State {
    fn update_tabs(&mut self, tabs: Vec<TabInfo>) {
        if let Some(active) = tabs.iter().find(|tab| tab.active) {
            self.active_tab = Some(active.position);
        }

        for tab in tabs {
            let tab_state = self.tabs.entry(tab.position).or_default();
            tab_state.active_layout = tab.active_swap_layout_name.clone();
            tab_state.is_dirty = tab.is_swap_layout_dirty;

            if let Some(layout_name) = tab_state.active_layout.as_ref() {
                if let Some(config) = self.config.as_ref() {
                    if let Some(bits) = config.state_bits.get(layout_name) {
                        tab_state.last_bits = Some(*bits);
                    }
                }
            }
        }
    }

    fn get_active_tab_state(&self) -> Option<&TabState> {
        self.active_tab.and_then(|pos| self.tabs.get(&pos))
    }
}

impl State {
    /// Handle tab update events
    pub fn on_tab_update(&mut self, tabs: Vec<TabInfo>) {
        if self.config.is_none() {
            return;
        }

        self.update_tabs(tabs);
    }

    /// Get the current bit pattern for the active tab
    fn current_bits(&self, config: &PluginConfig) -> Option<u64> {
        if let Some(tab_state) = self.get_active_tab_state() {
            if let Some(layout_name) = tab_state.active_layout.as_ref() {
                if let Some(bits) = config.state_bits.get(layout_name) {
                    return Some(*bits);
                }
            }

            if let Some(bits) = tab_state.last_bits {
                return Some(bits);
            }
        }

        config.default_bits()
    }

    /// Apply a command by name
    pub fn apply_command(&mut self, command_name: &str) {
        let Some(config) = self.config.as_ref() else {
            eprintln!("zjide-manager: plugin not configured yet");
            return;
        };

        let Some(command) = config.commands.get(command_name) else {
            eprintln!("zjide-manager: unknown trigger '{command_name}'");
            return;
        };

        let Some(current_bits) = self.current_bits(config) else {
            eprintln!("zjide-manager: unable to determine current layout bits");
            return;
        };

        let Some(target_bits) = config.resolve_target_bits(current_bits, command) else {
            eprintln!(
                "zjide-manager: trigger '{command_name}' references an unknown feature/state"
            );
            return;
        };

        let (target_layout, resolved_bits) = if let Some(layout) =
            config.bits_to_state.get(&target_bits)
        {
            (layout.clone(), target_bits)
        } else if let Some((layout, bits)) = config.closest_state(target_bits) {
            eprintln!(
                "zjide-manager: layout for mask {target_bits} missing, falling back to {layout}"
            );
            (layout, bits)
        } else {
            eprintln!("zjide-manager: no layouts available to satisfy trigger '{command_name}'");
            return;
        };

        go_to_swap_layout(&target_layout);
    }
}
