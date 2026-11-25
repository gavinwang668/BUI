//! Screen readers: the window's accessibility tree (the kit's widgets and the page) and the
//! requests a reader makes, end to end (no platform adapter: what one would be served).

use std::sync::Mutex;

use blenderui::a11y::{Action, Role, flags};
use blenderui::prelude::*;

/// What the page's listeners heard.
static HEARD: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn heard(s: impl Into<String>) {
    HEARD.lock().unwrap().push(s.into());
}

fn take_heard() -> Vec<String> {
    std::mem::take(&mut *HEARD.lock().unwrap())
}

struct Form;

impl Render for Form {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        html().child(head().child(title().child("Sign up"))).child(
            body()
                .child(h1().child("Create an account"))
                .child(
                    form()
                        .child(label().attr("for", "name").child("Your name"))
                        .child(input().id("name").attr("placeholder", "Ada")
                               .on_change(|e: &ChangeEvent, _, _| heard(format!("name={}", e.value))))
                        .child(label().child(input().id("agree").attr("type", "checkbox")).child("I agree"))
                        .child(input().id("count").attr("type", "number").attr("aria-label", "Seats")
                               .attr("min", "1").attr("max", "9").attr("value", "2"))
                        .child(button().id("send").child("Send").on_click(|_, _, _| heard("send"))),
                )
                .child(div().id("fancy").attr("role", "button").attr("aria-label", "Fancy")
                       .attr("tabindex", "0").child("★").on_click(|_, _, _| heard("fancy")))
                .child(a().attr("href", "#help").child("Help"))
                .child(ul().child(li().child("One")).child(li().child("Two")))
                .child(img().attr("src", "nothing.png").attr("alt", "Logo"))
                .child(div().attr("aria-hidden", "true").child("Hidden text"))
                .child(div().style("visibility: hidden").child("Invisible text"))
                .child(div().attr("role", "status").attr("aria-live", "polite").child("Saved"))
                .child(details().child(summary().child("More")).child(p().child("Extra"))),
        )
    }
}

fn open(cx: &mut App) -> WindowId {
    let opts = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds { origin: point(px(0.0), px(0.0)),
                                                            size: size(px(480.0), px(640.0)) })),
        ..Default::default()
    };
    let w = cx.open_window(opts, |_, cx| cx.new(|_| Form)).unwrap();
    cx.draw(w.id);
    w.id
}

#[test]
fn page_and_widgets_in_one_tree() {
    Application::headless().run(|cx| {
        let id = open(cx);
        let t = cx.accessibility_tree(id);
        assert_eq!(t.nodes[0].role, Role::Window);
        let doc = t.find(Role::Document, "Sign up").expect("the page, named by its <title>");
        assert_eq!(t.nodes[doc_index(&t, doc.id)].parent, Some(0));
        let h = t.find(Role::Heading, "Create an account").expect("h1");
        assert_eq!(h.level, 1);
        assert!(t.find(Role::Form, "").is_some());
        // form controls are the kit's widgets, named by their labels
        let name = t.find(Role::TextInput, "Your name").expect("<label for> names the field");
        assert!(name.has(flags::EDITABLE));
        let agree = t.find(Role::CheckBox, "I agree").expect("a wrapping <label> names the checkbox");
        assert!(agree.has(flags::CHECKABLE) && !agree.has(flags::CHECKED));
        let seats = t.find(Role::SpinButton, "Seats").expect("aria-label names the number field");
        let (v, lo, hi) = seats.numeric.expect("a numeric value");
        assert_eq!((v, lo, hi), (2.0, 1.0, 9.0));
        let send = t.find(Role::Button, "Send").expect("the button widget");
        assert!(send.has(flags::CLICKABLE | flags::FOCUSABLE));
        // ARIA
        let fancy = t.find(Role::Button, "Fancy").expect("role=button with aria-label");
        assert!(fancy.has(flags::FOCUSABLE | flags::CLICKABLE));
        assert!(t.find(Role::Link, "Help").is_some());
        let list = t.nodes.iter().position(|n| n.role == Role::List).expect("ul");
        assert_eq!(t.children(list).filter(|n| n.role == Role::ListItem).count(), 2);
        assert!(t.find(Role::Image, "Logo").is_some(), "alt names the image");
        assert!(t.find(Role::Label, "Hidden text").is_none(), "aria-hidden is left out");
        assert!(t.find(Role::Label, "Invisible text").is_none(), "visibility: hidden is left out");
        let status = t.nodes.iter().find(|n| n.role == Role::Status).expect("role=status");
        assert!(status.has(flags::LIVE_POLITE));
        let more = t.find(Role::Summary, "More").expect("summary");
        assert!(more.has(flags::EXPANDABLE) && !more.has(flags::EXPANDED));
        assert!(t.find(Role::Label, "Extra").is_none(), "a closed <details> hides its content");
        // every node lies inside the window, and ids are unique
        let mut ids: Vec<u64> = t.nodes.iter().map(|n| n.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), t.nodes.len());
    });
}

fn doc_index(t: &blenderui::a11y::AccessTree, id: u64) -> usize {
    t.nodes.iter().position(|n| n.id == id).unwrap()
}

#[test]
fn reader_requests_reach_the_elements() {
    Application::headless().run(|cx| {
        let id = open(cx);
        take_heard();
        let t = cx.accessibility_tree(id);
        // a click on the kit's button runs the page's on_click
        let send = t.find(Role::Button, "Send").unwrap().id;
        assert!(cx.accessibility_action(id, send, Action::Click, None, 0.0));
        // a click on an element of the page runs its on_click
        let fancy = t.find(Role::Button, "Fancy").unwrap().id;
        assert!(cx.accessibility_action(id, fancy, Action::Click, None, 0.0));
        assert_eq!(take_heard(), ["send", "fancy"]);
        // a value set by the reader is a change the page hears
        let name = t.find(Role::TextInput, "Your name").unwrap().id;
        assert!(cx.accessibility_action(id, name, Action::SetValue, Some("Grace"), 0.0));
        assert_eq!(take_heard(), ["name=Grace"]);
        let t = cx.accessibility_tree(id);
        let field = t.find(Role::TextInput, "Your name").unwrap();
        assert_eq!(field.value, "Grace");
        assert_eq!(field.id, name, "ids stay while the element does (the page rendered again)");
        assert_eq!(t.find(Role::Button, "Fancy").unwrap().id, fancy);
        // the checkbox toggles
        let agree = t.find(Role::CheckBox, "I agree").unwrap().id;
        assert!(cx.accessibility_action(id, agree, Action::Click, None, 0.0));
        assert!(cx.accessibility_tree(id).find(Role::CheckBox, "I agree").unwrap().has(flags::CHECKED));
        // increment the number field
        let seats = t.find(Role::SpinButton, "Seats").unwrap().id;
        assert!(cx.accessibility_action(id, seats, Action::Increment, None, 0.0));
        let seats_now = cx.accessibility_tree(id).find(Role::SpinButton, "Seats").unwrap().numeric;
        assert_eq!(seats_now, Some((3.0, 1.0, 9.0)));
        // focus moves to the element, and the tree says so
        assert!(cx.accessibility_action(id, fancy, Action::Focus, None, 0.0));
        let t = cx.accessibility_tree(id);
        assert_eq!(t.focused().map(|n| n.name.as_str()), Some("Fancy"));
        // a summary opens its details
        let more = t.find(Role::Summary, "More").unwrap().id;
        assert!(cx.accessibility_action(id, more, Action::Expand, None, 0.0));
        let t = cx.accessibility_tree(id);
        assert!(t.find(Role::Summary, "More").unwrap().has(flags::EXPANDED));
        assert!(t.find(Role::Label, "Extra").is_some());
        // an id that is not in the tree does nothing
        assert!(!cx.accessibility_action(id, 12345, Action::Click, None, 0.0));
    });
}

/// Blender's own widgets, outside any page: the kit describes them.
struct Kit;

impl Render for Kit {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .child(NumberSlider().attr("aria-label", "Opacity").min(0.0).max(10.0).value("5"))
            .child(Dropdown().attr("aria-label", "Quality").child(option().child("Low"))
                   .child(option().child("Medium")).child(option().child("High")))
    }
}

#[test]
fn blender_widgets() {
    Application::headless().run(|cx| {
        let opts = WindowOptions::default();
        let w = cx.open_window(opts, |_, cx| cx.new(|_| Kit)).unwrap();
        cx.draw(w.id);
        let t = cx.accessibility_tree(w.id);
        let slider = t.nodes.iter().find(|n| n.role == Role::Slider).expect("the number slider");
        let (v, lo, hi) = slider.numeric.unwrap();
        assert_eq!((v, lo, hi), (5.0, 0.0, 10.0));
        assert_eq!(slider.name, "Opacity");
        let combo = t.find(Role::ComboBox, "Quality").expect("the dropdown");
        assert_eq!(combo.value, "Low");
        assert!(combo.has(flags::EXPANDABLE) && !combo.has(flags::EXPANDED));
        // opening it shows its menu, whose rows are menu items
        let combo = combo.id;
        assert!(cx.accessibility_action(w.id, combo, Action::Expand, None, 0.0));
        let t = cx.accessibility_tree(w.id);
        assert!(t.nodes.iter().any(|n| n.role == Role::ComboBox && n.has(flags::EXPANDED)));
        let high = t.find(Role::MenuItem, "High").expect("the open menu's rows");
        assert!(cx.accessibility_action(w.id, high.id, Action::Click, None, 0.0));
        let t = cx.accessibility_tree(w.id);
        assert_eq!(t.nodes.iter().find(|n| n.role == Role::ComboBox).unwrap().value, "High");
        assert!(t.nodes.iter().all(|n| n.role != Role::Menu), "the menu closed");
    });
}

/// The kit checks a host's subtree at its boundary and keeps the last good one.
#[test]
fn bad_subtrees_are_refused() {
    use blenderui::sys;
    let node = |id, parent, role: Role, flags| sys::BKA11yNode { id, parent, role: role as u16, flags,
                                                                  ..sys::BKA11yNode::default() };
    // SAFETY: a fresh node of our own, freed at the end; arrays valid for each call.
    unsafe {
        let canvas = sys::BK_node_new(c"canvas".as_ptr());
        let set = |nodes: &[sys::BKA11yNode]| sys::BK_a11y_set_subtree(canvas, nodes.as_ptr(), i32::try_from(nodes.len()).unwrap());
        assert_eq!(set(&[node(1, -1, Role::Button, 0), node(2, 0, Role::Label, 0)]), 1);
        assert_eq!(set(&[node(1, -1, Role::Button, 0), node(1, -1, Role::Label, 0)]), 0, "the same id twice");
        assert_eq!(set(&[node(0, -1, Role::Button, 0)]), 0, "id 0");
        assert_eq!(set(&[node(1, 0, Role::Button, 0)]), 0, "a parent after its child");
        assert_eq!(set(&[node(1, -1, Role::Button, flags::CHECKED)]), 0, "checked without checkable");
        let mut bad = node(1, -1, Role::Button, 0);
        bad.role = 999;
        assert_eq!(set(&[bad]), 0, "an unknown role");
        let mut own = node(1, -1, Role::Button, 0);
        own.widget = canvas.cast();
        assert_eq!(set(&[own]), 0, "a node can't show itself");
        assert_eq!(sys::BK_a11y_set_subtree(canvas, std::ptr::null(), 2), 0, "no nodes");
        assert_eq!(sys::BK_a11y_set_subtree(canvas, std::ptr::null(), 0), 1, "none: the subtree goes");
        sys::BK_node_free(canvas);
    }
}
