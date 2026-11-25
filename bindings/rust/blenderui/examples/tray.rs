//! The desktop shell: a tray icon with a menu, notifications, the taskbar's progress and a
//! recent document. `cargo run --example tray`, then right-click the icon in the notification
//! area (Windows), the menu bar (macOS) or the panel's tray (Linux, with a StatusNotifierItem
//! host).

use blenderui::prelude::*;
use blenderui::shell::{self, MenuItem, ShellEvent, TaskbarProgress, Tray};

const STEP: u32 = 1;
const PAUSE: u32 = 2;
const NOTIFY: u32 = 3;
const QUIT: u32 = 4;

struct Demo {
    tray: Option<Tray>,
    progress: f32,
    paused: bool,
    clicks: u32,
}

impl Demo {
    fn menu(&self) {
        if let Some(tray) = &self.tray {
            let more = [MenuItem::action(NOTIFY, "Send a notification")];
            tray.set_menu(&[
                MenuItem::action(STEP, "Advance the progress"),
                MenuItem::check(PAUSE, "Paused", self.paused),
                MenuItem::submenu("More", &more),
                MenuItem::Separator,
                MenuItem::action(QUIT, "Quit"),
            ]);
        }
    }

    fn step(&mut self) {
        self.progress = if self.progress >= 1.0 { 0.0 } else { self.progress + 0.1 };
    }
}

impl Render for Demo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // the taskbar button follows the state each render
        window.set_taskbar_progress(match (self.progress, self.paused) {
            (p, _) if p <= 0.0 => TaskbarProgress::Hidden,
            (p, true) => TaskbarProgress::Paused(p),
            (p, false) => TaskbarProgress::Normal(p),
        });
        let f = shell::features();
        div().styled().flex().flex_col().gap_2().p_4()
            .child(h1().child("Desktop shell"))
            .child(p().child(format!(
                "tray {} · notifications {} · progress {} · recent files {}",
                f.tray(), f.notifications(), f.progress(), f.recent_documents())))
            .child(p().child(format!("Progress {:.0} % · tray clicks {}", self.progress * 100.0, self.clicks)))
            .child(button().child("Advance the progress").on_click(cx.listener(|this, _, _, cx| {
                this.step();
                cx.notify();
            })))
            .child(button().child("Notify").on_click(cx.listener(|_, _, _, _| {
                shell::notify("BUI", "Hello from the tray example", Some("BLENDER"));
            })))
            .child(button().child("Add this example's source to the recent documents").on_click(
                cx.listener(|_, _, _, _| {
                    shell::add_recent_document(file!());
                })))
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        shell::set_app_id("BUI.TrayExample");
        let win = cx
            .open_window(WindowOptions::default(), |_, cx| {
                cx.new(|_| Demo { tray: Tray::new(Some("BLENDER"), "BUI tray example"), progress: 0.0,
                                  paused: false, clicks: 0 })
            })
            .unwrap();
        let demo = win.root();
        demo.update(cx, |this, _| this.menu());
        cx.on_shell_event(move |event, cx| {
            match *event {
                ShellEvent::MenuItem { id: QUIT, .. } => return cx.quit(),
                ShellEvent::MenuItem { id: NOTIFY, .. } => {
                    shell::notify("BUI", "Sent from the tray menu", None);
                }
                ShellEvent::NotificationClicked(id) => println!("notification {} clicked", id.0),
                _ => {}
            }
            demo.update(cx, |this, cx| {
                match *event {
                    ShellEvent::MenuItem { id: STEP, .. } => this.step(),
                    ShellEvent::MenuItem { id: PAUSE, .. } => {
                        this.paused = !this.paused;
                        this.menu();
                    }
                    ShellEvent::TrayClicked { .. } | ShellEvent::TrayDoubleClicked { .. } => this.clicks += 1,
                    _ => {}
                }
                cx.notify();
            });
        });
    });
}
