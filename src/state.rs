use crate::config::{BitResolver, PluginConfig, StateResolver};
use std::collections::BTreeMap;
use zellij_tile::prelude::{
    focus_pane_with_id, next_swap_layout, previous_swap_layout, PaneId, PaneManifest, TabInfo,
};

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
    pub tracked_panes: BTreeMap<String, PaneId>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            config: None,
            tabs: BTreeMap::new(),
            active_tab: None,
            tracked_panes: BTreeMap::new(),
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
        self.update_tabs(tabs);
    }

    pub fn on_pane_update(&mut self, manifest: PaneManifest) {
        self.tracked_panes.clear();

        if let Some(active_tab) = self.active_tab {
            if let Some(panes) = manifest.panes.get(&active_tab) {
                for pane in panes {
                    let pane_id = match pane.is_plugin {
                        true => PaneId::Plugin(pane.id),
                        false => PaneId::Terminal(pane.id),
                    };
                    self.tracked_panes.insert(pane.title.clone(), pane_id);
                }
            }
        }
    }

    pub fn focus_pane(&self, name: &str) {
        if let Some(pane_id) = self.tracked_panes.get(name) {
            focus_pane_with_id(*pane_id, true, true);
        }
    }

    /// Focus a pane, toggling its feature visibility if not currently enabled
    pub fn focus_or_toggle_pane(&mut self, pane_name: &str) {
        let Some(config) = self.config.as_ref() else {
            eprintln!("zjide-manager: plugin not configured yet");
            return;
        };

        // Find feature that maps to this pane using existing feature_to_pane
        let feature_name = config.feature_to_pane.iter()
            .find(|(_, pane)| *pane == pane_name)
            .map(|(feature, _)| feature.clone());

        let Some(feature_name) = feature_name else {
            // No feature mapping, just try to focus the pane (fallback to focus-pane behavior)
            self.focus_pane(pane_name);
            return;
        };

        // Check current bits to see if the feature is enabled
        let Some(current_bits) = self.current_bits(config) else {
            eprintln!("zjide-manager: unable to determine current layout bits");
            return;
        };

        let Some(feature_bit) = config.bit_for_feature(&feature_name) else {
            eprintln!("zjide-manager: unknown feature '{feature_name}'");
            return;
        };

        let feature_enabled = (current_bits & feature_bit) != 0;

        if feature_enabled {
            // Feature is enabled, just focus the pane
            self.focus_pane(pane_name);
        } else {
            // Feature is not enabled, enable it (show) and then focus
            let target_bits = current_bits | feature_bit;

            let target_layout = if let Some(layout) = config.bits_to_state.get(&target_bits) {
                layout.clone()
            } else if let Some((layout, _)) = config.closest_state(target_bits) {
                eprintln!(
                    "zjide-manager: layout for mask {target_bits} missing, falling back to {layout}"
                );
                layout
            } else {
                eprintln!(
                    "zjide-manager: no layouts available to satisfy feature '{feature_name}'"
                );
                return;
            };

            self.navigate_to_layout(&target_layout);

            // Focus the pane after navigation
            self.focus_pane(pane_name);
        }
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

        let (target_layout, _resolved_bits) = if let Some(layout) =
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

        self.navigate_to_layout(&target_layout);

        // Intelligent Focus Logic
        let gained_bits = target_bits & !current_bits;
        let mut focused = false;

        // Priority: terminal > sidebar > others
        for feature in &["terminal", "sidebar"] {
            if let Some(bit_idx) = config.feature_to_bit.get(*feature) {
                let bit = 1u64 << bit_idx;
                if gained_bits & bit != 0 {
                    if let Some(pane_name) = config.feature_to_pane.get(*feature) {
                        if let Some(pane_id) = self.tracked_panes.get(pane_name) {
                            focus_pane_with_id(*pane_id, true, true);
                            focused = true;
                            break;
                        }
                    }
                }
            }
        }

        if !focused {
            // Check other gained features if any
            for (feature_name, bit_idx) in &config.feature_to_bit {
                if *feature_name == "terminal" || *feature_name == "sidebar" {
                    continue;
                }
                let bit = 1u64 << bit_idx;
                if gained_bits & bit != 0 {
                    if let Some(pane_name) = config.feature_to_pane.get(feature_name) {
                        if let Some(pane_id) = self.tracked_panes.get(pane_name) {
                            focus_pane_with_id(*pane_id, true, true);
                            focused = true;
                            break;
                        }
                    }
                }
            }
        }

        if !focused {
            if let Some(default_pane) = config.default_focus_pane.as_ref() {
                self.focus_pane(default_pane);
            }
        }
    }

    fn navigate_to_layout(&self, target_layout: &str) {
        let Some(config) = self.config.as_ref() else {
            return;
        };

        let layout_order: Vec<&String> = config.state_bits.keys().collect();

        let Some(target_idx) = layout_order.iter().position(|l| *l == target_layout) else {
            eprintln!(
                "zjide-manager: target layout '{}' not found in configuration",
                target_layout
            );
            return;
        };

        let current_layout = self
            .get_active_tab_state()
            .and_then(|ts| ts.active_layout.as_deref());

        let current_idx = if let Some(current) = current_layout {
            layout_order.iter().position(|l| *l == current)
        } else {
            None
        };

        let Some(current_idx) = current_idx else {
            eprintln!("zjide-manager: current layout unknown, cannot navigate with prev/next");
            return;
        };

        let total_layouts = layout_order.len();
        let distance = target_idx as i32 - current_idx as i32;

        if distance == 0 {
            return;
        }

        let use_next = if distance > 0 {
            distance <= (total_layouts as i32 / 2)
        } else {
            -distance > (total_layouts as i32 / 2)
        };

        if use_next {
            let steps = if distance > 0 {
                distance as usize
            } else {
                (total_layouts as i32 + distance) as usize
            };
            for _ in 0..steps {
                next_swap_layout();
            }
        } else {
            let steps = if distance < 0 {
                (-distance) as usize
            } else {
                (total_layouts as i32 - distance) as usize
            };
            for _ in 0..steps {
                previous_swap_layout();
            }
        }
    }
}
