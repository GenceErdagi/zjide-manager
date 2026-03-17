use crate::config::{BitResolver, PluginConfig, StateResolver};
use std::collections::BTreeMap;
use zellij_tile::prelude::{focus_pane_with_id, go_to_swap_layout, PaneId, PaneManifest, TabInfo};

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
    pub startup_applied: bool,
    pub editor_pane_id: Option<PaneId>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            config: None,
            tabs: BTreeMap::new(),
            active_tab: None,
            startup_applied: false,
            editor_pane_id: None,
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
        let startup_layout = self
            .config
            .as_ref()
            .and_then(|c| c.startup_layout.as_ref().cloned());

        self.update_tabs(tabs);

        if !self.startup_applied {
            if let Some(ref startup) = startup_layout {
                let config = self.config.as_ref().unwrap(); // Safe because startup_layout was Some
                if config.state_bits.contains_key(startup) {
                    go_to_swap_layout(startup);
                } else if config.state_bits.contains_key("BASE") {
                    eprintln!(
                        "zjide-manager: startup_layout '{startup}' not found, falling back to 'BASE'"
                    );
                    go_to_swap_layout("BASE");
                } else {
                    eprintln!(
                        "zjide-manager: startup_layout '{startup}' and fallback 'BASE' not found"
                    );
                }
            }
            self.startup_applied = true;
        }
    }

    pub fn on_pane_update(&mut self, manifest: PaneManifest) {
        let editor_pane_name = self
            .config
            .as_ref()
            .and_then(|c| c.editor_pane_name.as_ref());

        self.editor_pane_id = None;

        if let Some(editor_pane_name) = editor_pane_name {
            if let Some(active_tab) = self.active_tab {
                if let Some(panes) = manifest.panes.get(&active_tab) {
                    for pane in panes {
                        if pane.title == *editor_pane_name {
                            self.editor_pane_id = Some(match pane.is_plugin {
                                true => PaneId::Plugin(pane.id),
                                false => PaneId::Terminal(pane.id),
                            });
                            return;
                        }
                    }
                }
            }
        }
    }

    pub fn focus_editor(&self) {
        if let Some(pane_id) = self.editor_pane_id {
            focus_pane_with_id(pane_id, true, true);
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

        go_to_swap_layout(&target_layout);
    }
}
