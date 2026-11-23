//! A page a screen reader reads: a form with labels, buttons, a list, a live status line and
//! Blender widgets. `cargo run --example a11y`, then start Narrator (Ctrl+Win+Enter) or any UI
//! Automation client (Accessibility Insights, Inspect.exe): Tab and the reader's navigation keys
//! move through it; activating "Add" adds a row, and the status line announces it.
//! `--print` prints the tree a reader gets instead (headless).

use blenderui::prelude::*;

struct Page {
    tasks: Vec<String>,
    draft: String,
    status: String,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        html().child(head().child(title().child("Accessible tasks"))).child(
            body().p_4().flex().flex_col().gap_3()
                .child(h1().m_0().child("Tasks"))
                .child(
                    form().flex().gap_2().items_center()
                        .child(label().attr("for", "task").child("New task"))
                        .child(input().id("task").attr("placeholder", "What needs doing?").value(self.draft.clone())
                               .on_change(cx.listener(|this, e: &ChangeEvent, _, _| this.draft.clone_from(&e.value))))
                        .child(button().id("add").child("Add").on_click(cx.listener(|this, _, _, cx| {
                            let t = std::mem::take(&mut this.draft);
                            if !t.trim().is_empty() {
                                this.status = format!("Added \"{t}\"");
                                this.tasks.push(t);
                            }
                            cx.notify();
                        }))),
                )
                .child(h2().m_0().child("To do"))
                .child(ul().attr("aria-label", "To do").children(self.tasks.iter().enumerate().map(|(i, t)| {
                    li().flex().gap_2().items_center()
                        .child(label().child(input().input_type("checkbox")).child(t.clone()))
                        .child(button().attr("aria-label", format!("Remove {t}")).child("×")
                               .on_click(cx.listener(move |this, _, _, cx| {
                                   let t = this.tasks.remove(i);
                                   this.status = format!("Removed \"{t}\"");
                                   cx.notify();
                               })))
                })))
                .child(div().attr("role", "status").attr("aria-live", "polite").child(self.status.clone()))
                .child(
                    fieldset().child(legend().child("Blender widgets"))
                        .child(div().flex().gap_2().items_center()
                            .child(label().attr("for", "prio").child("Priority"))
                            .child(NumberSlider().id("prio").min(0.0).max(10.0).value("5"))
                            .child(label().attr("for", "kind").child("Kind"))
                            .child(Dropdown().id("kind").child(option().child("Work")).child(option().child("Home")))
                            .child(Checkbox().id("done").attr("aria-label", "Done")))),
        )
    }
}

fn main() {
    let print = std::env::args().any(|a| a == "--print");
    let app = if print { Application::headless() } else { Application::new() };
    app.run(move |cx| {
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.0), px(0.0)),
                                                                size: size(px(560.0), px(520.0)) })),
            ..Default::default()
        };
        let page = Page { tasks: vec!["Buy milk".into(), "Write docs".into()], draft: String::new(),
                          status: String::new() };
        let win = cx.open_window(opts, |_, cx| cx.new(|_| page)).unwrap();
        if print {
            cx.draw(win.id);
            let tree = cx.accessibility_tree(win.id);
            for n in &tree.nodes {
                let depth = std::iter::successors(n.parent, |&p| tree.nodes[p].parent).count();
                let focus = if n.id == tree.focus { "  (focused)" } else { "" };
                println!("{:indent$}{:?} {:?} {:?}{focus}", "", n.role, n.name, n.value, indent = depth * 2);
            }
        }
    });
}
