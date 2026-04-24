# zjide-manager

`zjide-manager` is a Zellij plugin designed to provide IDE-like capabilities by managing workspace layouts through stateful feature toggles.

Instead of manually cycling through various swap layouts, this plugin allows users to define specific "features" (such as a sidebar, terminal, or debug panel) and maps combinations of these active features to specific layouts. Toggling a feature on or off automatically transitions the workspace to the appropriate layout using the standard Zellij `previous_swap_layout` and `next_swap_layout` APIs.

## Core Concept

The plugin operates on a bitmask system:

1.  **Features:** You define available features (e.g., `sidebar`, `terminal`).
2.  **Layout Mapping:** You map specific layout names to combinations of enabled features.
    *   Example: Layout `BASE` might represent `sidebar=true, terminal=true`.
    *   Example: Layout `zen` might represent `sidebar=false, terminal=false`.
3.  **State Management:** Sending a command to toggle a feature updates the internal state, and the plugin automatically navigates to the appropriate layout using the shortest path via prev/next swap layout commands.

## Configuration

For demonstration purposes, example configuration files have been provided in the `demo-config` directory. These files illustrate how to configure the plugin, define layouts, and set up keybindings.

*   `demo-config/config.kdl`: Plugin configuration.
*   `demo-config/layouts/ide.kdl`: Base layout + swap layouts (all in one file).

### Example Plugin Configuration

```kdl
plugin location="file:/path/to/zjide-manager.wasm" {
    // Define the default starting layout
    default_layout "BASE"

    // Focus management
    default_focus_pane "Editor"
    pane_name.editor   "Editor"
    pane_name.terminal "Terminal"
    pane_name.sidebar  "File-Explorer"

    // Map layouts to feature flags
    layout.BASE        "sidebar=true, terminal=true"
    layout.no_sidebar  "sidebar=false, terminal=true"
    layout.no_terminal "sidebar=true, terminal=false"
    layout.zen         "sidebar=false, terminal=false"

    // Define triggers to control features
    trigger.toggle_sidebar  "toggle sidebar"
    trigger.toggle_terminal "toggle terminal"
    trigger.zen             "state zen"
}
```

## Focus Management

The plugin includes an intelligent focus management system that automatically shifts focus when layouts change:

1.  **Priority Focus:** When a feature is newly enabled (e.g., toggling the terminal on), the plugin prioritizes focusing that new pane.
    *   **Priority Order:** `terminal` > `sidebar` > other features.
2.  **Default Focus:** If no new features are enabled (e.g., toggling a feature off or just switching states), the plugin falls back to focusing the `default_focus_pane`.
3.  **Automatic Fallback:** If `default_focus_pane` is not explicitly set, it defaults to the value of `pane_name.editor`.

### Dynamic Focus via Pipe

You can manually trigger focus for any tracked pane using the `focus-pane` pipe message:

```kdl
bind "Alt f" {
    MessagePlugin "zjide-manager" {
        name "focus-pane"
        payload "Editor"
    }
}
```

### Smart Focus-Or-Toggle via Pipe

The `focus-or-toggle-pane` pipe message provides intelligent focus management:

- **If the pane's feature is visible**: Simply focuses the pane
- **If the pane's feature is hidden**: Enables the feature (shows it) then focuses the pane

This is useful for workflows like toggling sidebar from helix, or returning focus to editor from yazi:

```kdl
// Toggle sidebar visibility and focus it from helix
bind "Alt e" {
    MessagePlugin "zjide-manager" {
        name "focus-or-toggle-pane"
        payload "File-Explorer"
    }
}
```

**From terminal (for use in helix/yazi commands):**
```bash
# Toggle sidebar and focus it
zellij pipe --plugin file:/path/to/zjide-manager.wasm --name focus-or-toggle-pane -- "File-Explorer"

# Focus terminal (toggle if hidden)
zellij pipe --plugin file:/path/to/zjide-manager.wasm --name focus-or-toggle-pane -- "Terminal"

# Return to editor from yazi (just focus, no toggle needed)
zellij pipe --plugin file:/path/to/zjide-manager.wasm --name focus-pane -- "Editor"
```

### Keybindings

Keybindings send messages to the plugin to trigger state changes:

```kdl
// Traditional toggle (changes layout, focuses only if newly enabled)
bind "Alt e" {
    MessagePlugin "zjide-manager" {
        name "toggle_sidebar"
    }
}

// Smart toggle+focus (focuses if visible, shows then focuses if hidden)
bind "Alt e" {
    MessagePlugin "zjide-manager" {
        name "focus-or-toggle-pane"
        payload "File-Explorer"
    }
}
```

## Building

Requirements:
- Rust with `wasm32-wasip1` target: `rustup target add wasm32-wasip1`

```bash
# Debug build
cargo build --target wasm32-wasip1

# Release build (smaller, optimized)
cargo build --release --target wasm32-wasip1
```

The wasm file will be at `target/wasm32-wasip1/debug/zjide-manager.wasm` or `target/wasm32-wasip1/release/zjide-manager.wasm`.

## Testing / Demo

The `demo-config` directory contains a complete example setup:

```bash
# Build the plugin first
cargo build

# Run the demo (use --config to load demo config, -l for layout)
ZELLIJ_CONFIG=demo-config/config.kdl zellij -l demo-config/layouts/ide.kdl
```

**Keybindings:**
- `Alt e` - Toggle sidebar (File-Explorer)
- `Alt r` - Toggle terminal (r for right pane)

## Release

Releases are automatically built and published to GitHub Releases:

```bash
# Build release version locally
cargo build --release --target wasm32-wasip1

# The wasm is at:
# target/wasm32-wasip1/release/zjide-manager.wasm
```

Download pre-built releases from: https://github.com/GenceErdagi/zjide-manager/releases

**Using released version:**
```kdl
plugins {
    zjide-manager location="https://github.com/GenceErdagi/zjide-manager/releases/download/v0.2.0/zjide-manager.wasm" {
        default_layout "BASE"
        pane_name.editor   "Editor"
        pane_name.terminal "Terminal"
        pane_name.sidebar  "File-Explorer"
        layout.BASE        "sidebar=true, terminal=true"
        layout.zen         "sidebar=false, terminal=false"
        trigger.toggle_sidebar  "toggle sidebar"
        trigger.toggle_terminal "toggle terminal"
    }
}
```
