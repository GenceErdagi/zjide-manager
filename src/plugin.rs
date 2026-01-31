use crate::config::PluginConfig;
use crate::state::State;
use std::collections::BTreeMap;
use zellij_tile::prelude::*;

/// Trait for plugin lifecycle management
trait PluginLifecycle {
    fn initialize(&mut self, configuration: BTreeMap<String, String>);
    fn handle_event(&mut self, event: Event) -> bool;
    fn handle_pipe(&mut self, pipe_message: PipeMessage) -> bool;
    fn render_ui(&mut self, rows: usize, cols: usize);
}

impl PluginLifecycle for State {
    fn initialize(&mut self, configuration: BTreeMap<String, String>) {
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
        ]);
        subscribe(&[EventType::TabUpdate]);

        match PluginConfig::parse(&configuration) {
            Ok(config) => {
                self.config = Some(config);
            }
            Err(err) => eprintln!("zjide-manager: failed to parse configuration: {err}"),
        }
    }

    fn handle_event(&mut self, event: Event) -> bool {
        match event {
            Event::TabUpdate(tabs) => {
                self.on_tab_update(tabs);
                false
            }
            _ => false,
        }
    }

    fn handle_pipe(&mut self, pipe_message: PipeMessage) -> bool {
        self.apply_command(&pipe_message.name);
        false
    }

    fn render_ui(&mut self, _rows: usize, _cols: usize) {
        // No UI to render
    }
}

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.initialize(configuration);
    }

    fn update(&mut self, event: Event) -> bool {
        self.handle_event(event)
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        self.handle_pipe(pipe_message)
    }

    fn render(&mut self, rows: usize, cols: usize) {
        self.render_ui(rows, cols);
    }
}
