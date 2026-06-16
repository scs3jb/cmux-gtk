//! Dock — a right-side column of small terminal "controls" (lazygit, log
//! tails, watchers, …), each a Ghostty-backed terminal running a command from
//! `dock.json`.
//!
//! Config (merged, project wins, deduped by id):
//!   - `<workspace>/.cmux/dock.json`
//!   - `~/.config/cmux/dock.json`
//!
//! Each control: `id`, `title`, `command`, optional `cwd`, `height`.
//! The dock is built once per window and toggled via visibility, so its
//! terminals persist (they only spawn once the dock becomes visible).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

use gtk4::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

use crate::app::AppState;

thread_local! {
    /// window_id → dock Box, so a shortcut/palette can toggle it.
    static DOCKS: RefCell<HashMap<String, gtk4::Box>> = RefCell::new(HashMap::new());
}

#[derive(Debug, Clone, Deserialize)]
pub struct DockControl {
    pub id: String,
    #[serde(default)]
    pub title: Option<String>,
    pub command: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub height: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
struct DockJson {
    #[serde(default)]
    controls: Vec<DockControl>,
}

/// Load + merge dock controls for a workspace directory.
pub fn load(workspace_dir: &str) -> Vec<DockControl> {
    let mut paths = Vec::new();
    if !workspace_dir.is_empty() {
        paths.push(Path::new(workspace_dir).join(".cmux/dock.json"));
    }
    paths.push(crate::settings::config_dir().join("dock.json"));

    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for path in paths {
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for control in parse_controls(&content) {
            if control.id.is_empty() || control.command.trim().is_empty() {
                continue;
            }
            if seen.insert(control.id.clone()) {
                out.push(control);
            }
        }
    }
    out
}

/// Parse dock controls from a `dock.json` string. Accepts either
/// `{"controls": [...]}` or a bare `[...]` array.
fn parse_controls(content: &str) -> Vec<DockControl> {
    match serde_json::from_str::<DockJson>(content) {
        Ok(d) if !d.controls.is_empty() => d.controls,
        _ => serde_json::from_str::<Vec<DockControl>>(content).unwrap_or_default(),
    }
}

/// Build the dock Box for `window_id`, populated from `workspace_dir`. Returns
/// an empty (hidden) Box when there are no controls. Registered for toggling.
pub fn create_dock(window_id: Uuid, workspace_dir: &str, state: &Rc<AppState>) -> gtk4::Box {
    let dock = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    dock.add_css_class("dock-panel");
    dock.set_width_request(360);

    let controls = load(workspace_dir);
    if controls.is_empty() {
        dock.set_visible(false);
    } else {
        let header = gtk4::Label::new(Some("Dock"));
        header.add_css_class("dim-label");
        header.add_css_class("caption-heading");
        header.set_xalign(0.0);
        header.set_margin_start(8);
        header.set_margin_top(6);
        header.set_margin_bottom(2);
        dock.append(&header);

        for control in controls {
            dock.append(&build_control(window_id, &control, workspace_dir, state));
        }
        dock.set_visible(crate::settings::load().show_dock);
    }

    DOCKS.with(|m| {
        m.borrow_mut().insert(window_id.to_string(), dock.clone());
    });
    dock
}

fn build_control(
    window_id: Uuid,
    control: &DockControl,
    base_dir: &str,
    state: &Rc<AppState>,
) -> gtk4::Widget {
    let section = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    section.add_css_class("dock-control");

    let title = gtk4::Label::new(Some(
        control.title.as_deref().unwrap_or(control.id.as_str()),
    ));
    title.add_css_class("dock-control-title");
    title.add_css_class("caption");
    title.set_xalign(0.0);
    title.set_margin_start(8);
    title.set_margin_top(4);
    section.append(&title);

    let cwd = resolve_dir(control.cwd.as_deref(), base_dir);
    // The dock is built once per window, so a fresh id per control is fine.
    let _ = window_id;
    let surface_id = Uuid::new_v4();
    let surface = state.terminal_surface_for(surface_id, Some(&cwd), Some(&control.command));
    if let Some(parent) = surface.parent() {
        if let Some(parent_box) = parent.downcast_ref::<gtk4::Box>() {
            parent_box.remove(&surface);
        }
    }
    surface.set_hexpand(true);
    surface.set_vexpand(true);
    section.set_height_request(control.height.unwrap_or(220).clamp(80, 1000) as i32);
    section.append(&surface);
    section.upcast()
}

/// Toggle the dock visibility for `window_id`. Returns the new visibility.
pub fn toggle(window_id: Uuid) -> bool {
    DOCKS.with(|m| {
        if let Some(dock) = m.borrow().get(&window_id.to_string()) {
            // Empty docks (no controls / no children) stay hidden.
            if dock.first_child().is_none() {
                return false;
            }
            let visible = !dock.is_visible();
            dock.set_visible(visible);
            visible
        } else {
            false
        }
    })
}

/// Resolve a `cwd` field (`.`/empty → base, `~/x`, `/abs`, or relative).
fn resolve_dir(cwd: Option<&str>, base: &str) -> String {
    let Some(raw) = cwd else {
        return base.to_string();
    };
    let raw = raw.trim();
    if raw.is_empty() || raw == "." {
        return base.to_string();
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        return dirs::home_dir()
            .map(|h| h.join(rest).to_string_lossy().into_owned())
            .unwrap_or_else(|| raw.to_string());
    }
    if raw.starts_with('/') {
        return raw.to_string();
    }
    Path::new(base)
        .join(raw.strip_prefix("./").unwrap_or(raw))
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_controls_object_form() {
        let c = parse_controls(
            r#"{ "controls": [ { "id": "git", "title": "Git", "command": "lazygit", "height": 300 } ] }"#,
        );
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].id, "git");
        assert_eq!(c[0].command, "lazygit");
        assert_eq!(c[0].height, Some(300));
    }

    #[test]
    fn parses_controls_bare_array() {
        let c = parse_controls(r#"[ { "id": "logs", "command": "tail -f app.log" } ]"#);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].id, "logs");
        assert!(c[0].title.is_none());
    }

    #[test]
    fn resolves_dirs() {
        assert_eq!(resolve_dir(None, "/b"), "/b");
        assert_eq!(resolve_dir(Some("./x"), "/b"), "/b/x");
        assert_eq!(resolve_dir(Some("/abs"), "/b"), "/abs");
    }
}
