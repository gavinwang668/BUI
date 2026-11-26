//! A kit tree as AccessKit updates: the whole tree once, then only the nodes that changed.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use accesskit::{HasPopup, Invalid, Live, NodeId, Rect, Toggled, TreeId, TreeInfo, TreeUpdate};

use crate::node::{Node, Role, flags, text};

/// What was sent to the platform adapter, to send only what changed next time.
#[derive(Default)]
pub struct Tree {
    /// Each node sent last, by id: a hash of everything it showed.
    sent: HashMap<u64, u64>,
    /// The next `sent` (swapped in, so updates reuse both maps).
    next: HashMap<u64, u64>,
    /// Children of each node, flattened (`starts[i]..starts[i + 1]`), reused.
    kids: Vec<NodeId>,
    starts: Vec<u32>,
    fill: Vec<u32>,
}

impl Tree {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget what was sent: the next update is the whole tree (the adapter starts again).
    pub fn reset(&mut self) {
        self.sent.clear();
    }

    /// The update that brings the adapter to `nodes` (a whole tree, [`validate`]d):
    /// every node with `full` or after [`reset`](Self::reset), else the new and changed ones.
    /// `focus` is the focused node's id (the root when it is not in the tree).
    ///
    /// # Safety
    /// `nodes` passed `validate(nodes, Shape::Tree)`, and their strings are valid for the call.
    ///
    /// [`validate`]: crate::node::validate
    #[must_use]
    pub unsafe fn update(&mut self, nodes: &[Node], focus: u64, full: bool) -> TreeUpdate {
        let full = full || self.sent.is_empty();
        self.children(nodes);
        self.next.clear();
        let mut out = Vec::new();
        for (i, n) in nodes.iter().enumerate() {
            let kids = &self.kids[self.starts[i] as usize..self.starts[i + 1] as usize];
            // SAFETY: the caller's contract.
            let hash = unsafe { hash_node(n, kids) };
            self.next.insert(n.id, hash);
            if full || self.sent.get(&n.id) != Some(&hash) {
                // SAFETY: the caller's contract.
                out.push((NodeId(n.id), unsafe { convert(n, kids) }));
            }
        }
        std::mem::swap(&mut self.sent, &mut self.next);
        let root = NodeId(nodes.first().map_or(0, |n| n.id));
        let focus = if self.sent.contains_key(&focus) { NodeId(focus) } else { root };
        TreeUpdate {
            nodes: out,
            tree: full.then(|| TreeInfo {
                root,
                toolkit_name: Some("BUI".into()),
                toolkit_version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
            tree_id: TreeId::ROOT,
            focus,
        }
    }

    /// Fill `kids` / `starts` with each node's children in order (a counting sort: no allocation
    /// once the buffers have grown).
    fn children(&mut self, nodes: &[Node]) {
        self.starts.clear();
        self.starts.resize(nodes.len() + 1, 0);
        for n in nodes.iter().filter(|n| n.parent >= 0) {
            self.starts[n.parent as usize + 1] += 1;
        }
        for i in 0..nodes.len() {
            self.starts[i + 1] += self.starts[i];
        }
        self.kids.clear();
        self.kids.resize(nodes.len(), NodeId(0));
        self.fill.clear();
        self.fill.extend_from_slice(&self.starts[..nodes.len()]);
        for n in nodes.iter().filter(|n| n.parent >= 0) {
            let at = &mut self.fill[n.parent as usize];
            self.kids[*at as usize] = NodeId(n.id);
            *at += 1;
        }
    }
}

/// Everything a node shows, hashed (equal hashes: nothing to send).
unsafe fn hash_node(n: &Node, kids: &[NodeId]) -> u64 {
    let mut h = std::hash::DefaultHasher::new();
    (n.role, n.level, n.flags).hash(&mut h);
    for p in [n.name, n.description, n.value] {
        // SAFETY: the caller's contract.
        unsafe { text(p) }.hash(&mut h);
    }
    for v in [n.x, n.y, n.w, n.h] {
        v.to_bits().hash(&mut h);
    }
    if n.flags & flags::NUMERIC != 0 {
        for v in [n.num, n.min, n.max, n.step] {
            v.to_bits().hash(&mut h);
        }
    }
    for k in kids {
        k.0.hash(&mut h);
    }
    h.finish()
}

/// One kit node as an AccessKit node.
unsafe fn convert(n: &Node, kids: &[NodeId]) -> accesskit::Node {
    use accesskit::Action as A;
    let f = n.flags;
    let has = |bit: u32| f & bit != 0;
    let role = Role::from_raw(n.role).unwrap_or(Role::Generic);
    let mut node = accesskit::Node::new(if role == Role::Button && has(flags::DEFAULT) {
        accesskit::Role::DefaultButton
    } else {
        role.accesskit()
    });
    // SAFETY (the three): the caller's contract.
    let (name, value) = unsafe { (text(n.name), text(n.value)) };
    if role == Role::Label {
        // AccessKit reads a label's text from its value
        if let Some(s) = name.or(value) {
            node.set_value(s);
        }
    } else {
        if let Some(s) = name {
            node.set_label(s);
        }
        // an editable field has a value (UI Automation's Value pattern), empty or not
        if let Some(s) = value.or_else(|| has(flags::EDITABLE).then_some("")) {
            node.set_value(s);
        }
    }
    if let Some(s) = unsafe { text(n.description) } {
        node.set_description(s);
    }
    if n.w > 0.0 || n.h > 0.0 {
        node.set_bounds(Rect {
            x0: f64::from(n.x),
            y0: f64::from(n.y),
            x1: f64::from(n.x + n.w),
            y1: f64::from(n.y + n.h),
        });
        node.add_action(A::ScrollIntoView);
    }
    if !kids.is_empty() {
        node.set_children(kids);
    }
    if n.level > 0 {
        node.set_level(usize::from(n.level));
    }
    if has(flags::DISABLED) {
        node.set_disabled();
    }
    if has(flags::CHECKABLE) {
        node.set_toggled(if has(flags::MIXED) {
            Toggled::Mixed
        } else {
            Toggled::from(has(flags::CHECKED))
        });
    }
    if has(flags::EXPANDABLE) {
        node.set_expanded(has(flags::EXPANDED));
    }
    if has(flags::SELECTABLE) {
        node.set_selected(has(flags::SELECTED));
    }
    if has(flags::READONLY) {
        node.set_read_only();
    }
    if has(flags::REQUIRED) {
        node.set_required();
    }
    if has(flags::MULTISELECTABLE) {
        node.set_multiselectable();
    }
    if has(flags::LIVE_POLITE) {
        node.set_live(Live::Polite);
    } else if has(flags::LIVE_ASSERTIVE) {
        node.set_live(Live::Assertive);
    }
    if has(flags::POPUP) {
        node.set_has_popup(HasPopup::Menu);
    }
    if has(flags::MODAL) {
        node.set_modal();
    }
    if has(flags::INVALID) {
        node.set_invalid(Invalid::True);
    }
    if has(flags::VISITED) {
        node.set_visited();
    }
    if has(flags::CLIPS) {
        node.set_clips_children();
    }
    if has(flags::NUMERIC) {
        node.set_numeric_value(n.num);
        if n.min.is_finite() {
            node.set_min_numeric_value(n.min);
        }
        if n.max.is_finite() {
            node.set_max_numeric_value(n.max);
        }
        if n.step > 0.0 {
            node.set_numeric_value_step(n.step);
        }
    }
    // what a reader may ask for
    let live = !has(flags::DISABLED);
    let writable = live && !has(flags::READONLY);
    if has(flags::FOCUSABLE) && live {
        node.add_action(A::Focus);
    }
    if has(flags::CLICKABLE) && live {
        node.add_action(A::Click);
    }
    if writable && (has(flags::EDITABLE) || has(flags::NUMERIC)) {
        node.add_action(A::SetValue);
    }
    if writable && has(flags::NUMERIC) {
        node.add_action(A::Increment);
        node.add_action(A::Decrement);
    }
    if has(flags::EXPANDABLE) && live {
        node.add_action(if has(flags::EXPANDED) { A::Collapse } else { A::Expand });
    }
    if has(flags::CONTEXT_MENU) && live {
        node.add_action(A::ShowContextMenu);
    }
    node
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{Shape, validate};

    fn node(id: u64, parent: i32, role: Role, name: &'static std::ffi::CStr) -> Node {
        Node { id, parent, role: role as u16, name: name.as_ptr(), w: 10.0, h: 10.0, ..Node::default() }
    }

    fn page() -> Vec<Node> {
        let mut check = node(4, 0, Role::CheckBox, c"Agree");
        check.flags = flags::CHECKABLE | flags::CHECKED | flags::FOCUSABLE | flags::CLICKABLE;
        let mut num = node(5, 0, Role::SpinButton, c"Count");
        num.flags = flags::NUMERIC | flags::FOCUSABLE;
        (num.num, num.min, num.max, num.step) = (3.0, 0.0, 10.0, 1.0);
        vec![node(1, -1, Role::Window, c"App"), node(2, 0, Role::Button, c"OK"), node(3, 0, Role::Label, c"Hello"),
             check, num]
    }

    fn update(t: &mut Tree, nodes: &[Node], focus: u64) -> TreeUpdate {
        // SAFETY: static strings; checked first like the ABI does.
        unsafe {
            validate(nodes, Shape::Tree).unwrap();
            t.update(nodes, focus, false)
        }
    }

    #[test]
    fn first_update_is_the_whole_tree() {
        let mut t = Tree::new();
        let u = update(&mut t, &page(), 2);
        assert_eq!(u.nodes.len(), 5);
        let info = u.tree.expect("tree info on the first update");
        assert_eq!(info.root, NodeId(1));
        assert_eq!(info.toolkit_name.as_deref(), Some("BUI"));
        assert_eq!(u.focus, NodeId(2));
        let (_, root) = &u.nodes[0];
        assert_eq!(root.role(), accesskit::Role::Window);
        assert_eq!(root.children(), &[NodeId(2), NodeId(3), NodeId(4), NodeId(5)]);
        let (_, check) = &u.nodes[3];
        assert_eq!(check.role(), accesskit::Role::CheckBox);
        assert_eq!(check.toggled(), Some(Toggled::True));
        assert!(check.supports_action(accesskit::Action::Click));
        let (_, num) = &u.nodes[4];
        assert_eq!((num.numeric_value(), num.min_numeric_value(), num.max_numeric_value()),
                   (Some(3.0), Some(0.0), Some(10.0)));
        assert!(num.supports_action(accesskit::Action::Increment));
        assert!(num.supports_action(accesskit::Action::SetValue));
        assert_eq!(num.label(), Some("Count"));
    }

    #[test]
    fn later_updates_send_only_changes() {
        let mut t = Tree::new();
        let mut nodes = page();
        let _ = update(&mut t, &nodes, 1);
        let same = update(&mut t, &nodes, 1);
        assert!(same.nodes.is_empty() && same.tree.is_none(), "nothing changed: nothing sent");
        // one value changes: that node alone, with the same id
        nodes[4].num = 4.0;
        let u = update(&mut t, &nodes, 1);
        assert_eq!(u.nodes.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [NodeId(5)]);
        // a node goes: its parent alone (the child left its children)
        nodes.remove(2);
        nodes[2].parent = 0;
        nodes[3].parent = 0;
        let u = update(&mut t, &nodes, 1);
        assert_eq!(u.nodes.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [NodeId(1)]);
        assert_eq!(u.nodes[0].1.children(), &[NodeId(2), NodeId(4), NodeId(5)]);
        // a node comes: it and its parent
        nodes.push(node(9, 1, Role::Label, c"Inner"));
        let u = update(&mut t, &nodes, 9);
        assert_eq!(u.nodes.iter().map(|(id, _)| *id).collect::<Vec<_>>(), [NodeId(2), NodeId(9)]);
        assert_eq!(u.focus, NodeId(9));
        // focus on a node that is not there: the root
        assert_eq!(update(&mut t, &nodes, 77).focus, NodeId(1));
        // the adapter started again: everything
        t.reset();
        assert_eq!(update(&mut t, &nodes, 1).nodes.len(), nodes.len());
    }

    #[test]
    fn labels_and_fields_carry_values() {
        let mut field = node(2, 0, Role::TextInput, c"Name");
        field.flags = flags::EDITABLE;
        let mut t = Tree::new();
        let u = update(&mut t, &[node(1, -1, Role::Window, c"App"), field, node(3, 0, Role::Label, c"Hello")], 1);
        assert_eq!(u.nodes[1].1.value(), Some(""), "an empty field still has a value");
        assert_eq!(u.nodes[1].1.label(), Some("Name"));
        assert_eq!((u.nodes[2].1.value(), u.nodes[2].1.label()), (Some("Hello"), None), "a label's text is its value");
    }

    #[test]
    fn states_map_to_accesskit() {
        let mut n = node(1, -1, Role::Button, c"Save");
        n.flags = flags::DEFAULT | flags::DISABLED | flags::CLICKABLE | flags::EXPANDABLE | flags::POPUP;
        let mut t = Tree::new();
        let u = update(&mut t, &[n], 1);
        let b = &u.nodes[0].1;
        assert_eq!(b.role(), accesskit::Role::DefaultButton);
        assert!(b.is_disabled());
        assert!(!b.supports_action(accesskit::Action::Click), "disabled: no actions");
        assert_eq!(b.is_expanded(), Some(false));
        assert_eq!(b.has_popup(), Some(HasPopup::Menu));
    }
}
