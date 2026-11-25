//! The desktop shell for the JavaScript runtimes (`blenderui::shell`): tray icons by JavaScript's
//! ids, their menus, notifications, a window's taskbar progress and the recent documents. Events
//! go to the windows' sink: `tray` (`target` the tray, `button` 1 left / 2 middle / 3 right, 0 a
//! double click), `traymenu` (`target` the tray, `number` the item's id) and `notification`
//! (`number` its id).

use std::borrow::Cow;

use blenderui::prelude::*;
use blenderui::shell::{self, MenuItem, ShellEvent, TaskbarProgress, Tray};

use super::{Event, UiState, WindowSink};

/// A menu item as JavaScript sends it.
#[derive(Clone, Debug, Default)]
pub struct TrayItem {
    /// What `traymenu` reports.
    pub id: u32,
    pub label: String,
    pub kind: TrayItemKind,
    pub checked: bool,
    pub enabled: bool,
    pub submenu: Vec<TrayItem>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TrayItemKind {
    #[default]
    Normal,
    Checkbox,
    Separator,
    Submenu,
}

impl TrayItemKind {
    /// Electron's `type` names; `None` for any other.
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "normal" => Self::Normal,
            "checkbox" => Self::Checkbox,
            "separator" => Self::Separator,
            "submenu" => Self::Submenu,
            _ => return None,
        })
    }
}

fn items(js: &[TrayItem]) -> Vec<MenuItem<'_>> {
    js.iter()
        .map(|it| {
            let item = match it.kind {
                TrayItemKind::Normal => MenuItem::action(it.id, &it.label),
                TrayItemKind::Checkbox => MenuItem::check(it.id, &it.label, it.checked),
                TrayItemKind::Separator => MenuItem::Separator,
                TrayItemKind::Submenu => MenuItem::submenu(&it.label, Cow::Owned(items(&it.submenu))),
            };
            if it.enabled { item } else { item.disabled() }
        })
        .collect()
}

/// The live trays, by JavaScript's id.
#[derive(Default)]
struct Trays(Vec<(u32, Tray)>);

fn trays(cx: &mut App) -> &mut Vec<(u32, Tray)> {
    if cx.try_global::<Trays>().is_none() {
        cx.set_global(Trays::default());
        cx.on_shell_event(|event, cx| {
            let ev = match *event {
                ShellEvent::TrayClicked { tray, button } => tray_event(cx, tray, "tray", u32::from(button), 0.0),
                ShellEvent::TrayDoubleClicked { tray } => tray_event(cx, tray, "tray", 0, 0.0),
                ShellEvent::MenuItem { tray, id } => tray_event(cx, tray, "traymenu", 0, f64::from(id)),
                ShellEvent::NotificationClicked(id) => {
                    Some(Event { kind: "notification".into(), number: f64::from(id.0), ..Default::default() })
                }
            };
            if let Some(ev) = ev {
                cx.global::<WindowSink>().0(ev);
            }
        });
    }
    &mut cx.global_mut::<Trays>().0
}

fn tray_event(cx: &App, tray: shell::TrayId, kind: &str, button: u32, number: f64) -> Option<Event> {
    let (id, _) = cx.try_global::<Trays>()?.0.iter().find(|(_, t)| t.id() == tray)?;
    Some(Event { kind: kind.into(), target: *id, button, number, ..Default::default() })
}

fn with_tray<R>(cx: &mut App, id: u32, f: impl FnOnce(&Tray) -> R) -> Option<R> {
    trays(cx).iter().find(|(i, _)| *i == id).map(|(_, t)| f(t))
}

/// Tray `id` (chosen by JavaScript); false where the system has no tray.
pub fn tray_new(cx: &mut App, id: u32, icon: Option<&str>, tooltip: &str) -> bool {
    let list = trays(cx);
    list.retain(|(i, _)| *i != id);
    Tray::new(icon, tooltip).map(|t| list.push((id, t))).is_some()
}

pub fn tray_set_icon(cx: &mut App, id: u32, icon: Option<&str>) -> bool {
    with_tray(cx, id, |t| t.set_icon(icon)).unwrap_or(false)
}

pub fn tray_set_tooltip(cx: &mut App, id: u32, tooltip: &str) -> bool {
    with_tray(cx, id, |t| t.set_tooltip(tooltip)).unwrap_or(false)
}

pub fn tray_set_menu(cx: &mut App, id: u32, menu: &[TrayItem]) -> bool {
    with_tray(cx, id, |t| t.set_menu(&items(menu))).unwrap_or(false)
}

pub fn tray_free(cx: &mut App, id: u32) {
    trays(cx).retain(|(i, _)| *i != id);
}

/// A notification's id; `None` when it can't show. Its clicks come as `notification` events.
pub fn notify(cx: &mut App, title: &str, body: &str, icon: Option<&str>) -> Option<u32> {
    trays(cx); // listens to the shell
    shell::notify(title, body, icon).map(|n| n.0)
}

/// Window `window`'s taskbar progress: `mode` "normal", "indeterminate", "paused", "error" or
/// "none" (Electron's `setProgressBar` modes); false for another mode or no such window.
pub fn set_progress(cx: &mut App, window: u32, mode: &str, value: f64) -> bool {
    let v = value as f32;
    let progress = match mode {
        "none" => TaskbarProgress::Hidden,
        "normal" => TaskbarProgress::Normal(v),
        "indeterminate" => TaskbarProgress::Indeterminate,
        "paused" => TaskbarProgress::Paused(v),
        "error" => TaskbarProgress::Error(v),
        _ => return false,
    };
    let Some(h) = cx.global::<UiState>().windows.get(&window) else { return false };
    let id = h.id;
    cx.window(id).is_some_and(|w| w.set_taskbar_progress(progress))
}

/// [tray, notifications, progress, recent documents]: what this build and system have.
pub fn features() -> [bool; 4] {
    let f = shell::features();
    [f.tray(), f.notifications(), f.progress(), f.recent_documents()]
}
