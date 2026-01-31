mod commands;
mod config;
mod plugin;
mod state;

use zellij_tile::prelude::*;

use state::State;

register_plugin!(State);
