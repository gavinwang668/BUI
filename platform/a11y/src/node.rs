//! The kit's flat accessibility nodes (`BKA11yNode` in kit/include/BUI_kit.h) and their checks.
//!
//! The C header is the source of truth for the layout and the numbers; `tests::header_agrees`
//! reads it and fails when this mirror drifts from it.

use std::ffi::{CStr, c_char};

/// `BK_ROLE_*`: one table, each role with its C name and its AccessKit role.
macro_rules! roles {
    ($($name:ident = $c:literal => $ak:ident,)*) => {
        /// What a node is (`BK_ROLE_*`).
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(u16)]
        pub enum Role {
            $($name,)*
        }

        impl Role {
            /// Every role, in `BK_ROLE_*` order.
            pub const ALL: &'static [Self] = &[$(Self::$name,)*];

            /// The `BK_ROLE_*` name (without the prefix).
            #[must_use]
            pub const fn c_name(self) -> &'static str {
                match self {
                    $(Self::$name => $c,)*
                }
            }

            /// The AccessKit role.
            #[must_use]
            pub const fn accesskit(self) -> accesskit::Role {
                match self {
                    $(Self::$name => accesskit::Role::$ak,)*
                }
            }

            /// The role numbered `n`, if any.
            #[must_use]
            pub fn from_raw(n: u16) -> Option<Self> {
                Self::ALL.get(usize::from(n)).copied()
            }
        }
    };
}

roles! {
    Generic = "GENERIC" => GenericContainer,
    Window = "WINDOW" => Window,
    Document = "DOCUMENT" => Document,
    Group = "GROUP" => Group,
    Pane = "PANE" => Pane,
    Label = "LABEL" => Label,
    Paragraph = "PARAGRAPH" => Paragraph,
    Heading = "HEADING" => Heading,
    Link = "LINK" => Link,
    Button = "BUTTON" => Button,
    CheckBox = "CHECKBOX" => CheckBox,
    Radio = "RADIO" => RadioButton,
    Switch = "SWITCH" => Switch,
    TextInput = "TEXT_INPUT" => TextInput,
    MultilineTextInput = "MULTILINE_TEXT_INPUT" => MultilineTextInput,
    SearchInput = "SEARCH_INPUT" => SearchInput,
    PasswordInput = "PASSWORD_INPUT" => PasswordInput,
    SpinButton = "SPIN_BUTTON" => SpinButton,
    Slider = "SLIDER" => Slider,
    ComboBox = "COMBO_BOX" => ComboBox,
    ListBox = "LIST_BOX" => ListBox,
    ListBoxOption = "LIST_BOX_OPTION" => ListBoxOption,
    List = "LIST" => List,
    ListItem = "LIST_ITEM" => ListItem,
    Tree = "TREE" => Tree,
    TreeItem = "TREE_ITEM" => TreeItem,
    Table = "TABLE" => Table,
    Row = "ROW" => Row,
    Cell = "CELL" => Cell,
    ColumnHeader = "COLUMN_HEADER" => ColumnHeader,
    RowHeader = "ROW_HEADER" => RowHeader,
    Menu = "MENU" => Menu,
    MenuBar = "MENU_BAR" => MenuBar,
    MenuItem = "MENU_ITEM" => MenuItem,
    MenuItemCheckBox = "MENU_ITEM_CHECKBOX" => MenuItemCheckBox,
    MenuItemRadio = "MENU_ITEM_RADIO" => MenuItemRadio,
    TabList = "TAB_LIST" => TabList,
    Tab = "TAB" => Tab,
    TabPanel = "TAB_PANEL" => TabPanel,
    Progress = "PROGRESS" => ProgressIndicator,
    Meter = "METER" => Meter,
    Image = "IMAGE" => Image,
    ColorWell = "COLOR_WELL" => ColorWell,
    ScrollBar = "SCROLL_BAR" => ScrollBar,
    ScrollView = "SCROLL_VIEW" => ScrollView,
    Toolbar = "TOOLBAR" => Toolbar,
    Dialog = "DIALOG" => Dialog,
    AlertDialog = "ALERT_DIALOG" => AlertDialog,
    Alert = "ALERT" => Alert,
    Status = "STATUS" => Status,
    Log = "LOG" => Log,
    Navigation = "NAVIGATION" => Navigation,
    Main = "MAIN" => Main,
    Banner = "BANNER" => Banner,
    ContentInfo = "CONTENT_INFO" => ContentInfo,
    Complementary = "COMPLEMENTARY" => Complementary,
    Region = "REGION" => Region,
    Form = "FORM" => Form,
    Search = "SEARCH" => Search,
    Article = "ARTICLE" => Article,
    Figure = "FIGURE" => Figure,
    Details = "DETAILS" => Details,
    Summary = "SUMMARY" => DisclosureTriangle,
    Canvas = "CANVAS" => Canvas,
    Video = "VIDEO" => Video,
    RadioGroup = "RADIO_GROUP" => RadioGroup,
    Tooltip = "TOOLTIP" => Tooltip,
    Code = "CODE" => Code,
    Blockquote = "BLOCKQUOTE" => Blockquote,
    Caption = "CAPTION" => Caption,
    Legend = "LEGEND" => Legend,
    Term = "TERM" => Term,
    Definition = "DEFINITION" => Definition,
    DescriptionList = "DESCRIPTION_LIST" => DescriptionList,
    Time = "TIME" => Time,
}

/// `BK_A11Y_*` node flags: states and abilities.
pub mod flags {
    pub const FOCUSABLE: u32 = 1 << 0;
    pub const FOCUSED: u32 = 1 << 1;
    pub const DISABLED: u32 = 1 << 2;
    pub const CHECKABLE: u32 = 1 << 3;
    pub const CHECKED: u32 = 1 << 4;
    pub const MIXED: u32 = 1 << 5;
    pub const EXPANDABLE: u32 = 1 << 6;
    pub const EXPANDED: u32 = 1 << 7;
    pub const SELECTABLE: u32 = 1 << 8;
    pub const SELECTED: u32 = 1 << 9;
    pub const CLICKABLE: u32 = 1 << 10;
    pub const EDITABLE: u32 = 1 << 11;
    pub const NUMERIC: u32 = 1 << 12;
    pub const READONLY: u32 = 1 << 13;
    pub const REQUIRED: u32 = 1 << 14;
    pub const MULTISELECTABLE: u32 = 1 << 15;
    pub const LIVE_POLITE: u32 = 1 << 16;
    pub const LIVE_ASSERTIVE: u32 = 1 << 17;
    pub const POPUP: u32 = 1 << 18;
    pub const MODAL: u32 = 1 << 19;
    pub const INVALID: u32 = 1 << 20;
    pub const VISITED: u32 = 1 << 21;
    pub const CONTEXT_MENU: u32 = 1 << 22;
    pub const CLIPS: u32 = 1 << 23;
    pub const DEFAULT: u32 = 1 << 24;
    pub const FLAGS_ALL: u32 = (1 << 25) - 1;
}

/// `BKA11yNode`: one node of a flat tree (parents before children, siblings in reading order).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub id: u64,
    /// Index of the parent; -1: the root (a whole tree) or a child of the holding node (a subtree).
    pub parent: i32,
    /// `BK_ROLE_*` ([`Role`]).
    pub role: u16,
    pub level: u16,
    /// `BK_A11Y_*` ([`flags`]).
    pub flags: u32,
    pub name: *const c_char,
    pub description: *const c_char,
    pub value: *const c_char,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub num: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    /// A kit node (`BKNode *`) shown here; only in a host's subtree.
    pub widget: *mut std::ffi::c_void,
}

// the C side asserts the same size (kit/intern/a11y/a11y.c)
const _: () = assert!(size_of::<Node>() == 72 + 4 * size_of::<usize>());

impl Default for Node {
    fn default() -> Self {
        Self {
            id: 0,
            parent: -1,
            role: Role::Generic as u16,
            level: 0,
            flags: 0,
            name: std::ptr::null(),
            description: std::ptr::null(),
            value: std::ptr::null(),
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
            num: 0.0,
            min: f64::NAN,
            max: f64::NAN,
            step: 0.0,
            widget: std::ptr::null_mut(),
        }
    }
}

/// `BK_A11Y_CLICK` ...: what a screen reader asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Action {
    Click = 0,
    Focus,
    SetValue,
    Increment,
    Decrement,
    Expand,
    Collapse,
    ScrollIntoView,
    ContextMenu,
}

impl Action {
    /// The action numbered `n`, if any.
    #[must_use]
    pub const fn from_raw(n: i32) -> Option<Self> {
        Some(match n {
            0 => Self::Click,
            1 => Self::Focus,
            2 => Self::SetValue,
            3 => Self::Increment,
            4 => Self::Decrement,
            5 => Self::Expand,
            6 => Self::Collapse,
            7 => Self::ScrollIntoView,
            8 => Self::ContextMenu,
            _ => return None,
        })
    }

    /// The kit's action for an AccessKit one, if it has it.
    #[must_use]
    pub const fn from_accesskit(a: accesskit::Action) -> Option<Self> {
        Some(match a {
            accesskit::Action::Click => Self::Click,
            accesskit::Action::Focus => Self::Focus,
            accesskit::Action::SetValue => Self::SetValue,
            accesskit::Action::Increment => Self::Increment,
            accesskit::Action::Decrement => Self::Decrement,
            accesskit::Action::Expand => Self::Expand,
            accesskit::Action::Collapse => Self::Collapse,
            accesskit::Action::ScrollIntoView => Self::ScrollIntoView,
            accesskit::Action::ShowContextMenu => Self::ContextMenu,
            _ => return None,
        })
    }
}

/// `BKA11yRequest`: an action for a node.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Request {
    pub id: u64,
    /// `BK_A11Y_CLICK` ... ([`Action`]).
    pub action: i32,
    pub num: f64,
    /// `SET_VALUE`'s text (UTF-8), NULL: `num`.
    pub text: *const c_char,
}

/// Why a flat tree was refused (`bui_a11y_validate`'s nonzero results, in this order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Invalid {
    /// A null array with nodes, or a negative count.
    Array = 1,
    /// An id of 0, or one used twice.
    Id,
    /// A parent that does not come before its child; a whole tree's root that is not first.
    Parent,
    Role,
    /// Unknown flags, or a state without its ability (CHECKED without CHECKABLE ...).
    Flags,
    /// A string that is not UTF-8.
    Utf8,
    /// A whole tree with a widget pointer (only subtrees show kit nodes).
    Widget,
}

/// Which kind of flat tree is checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// A whole tree: one root, first; no widgets.
    Tree,
    /// A host's subtree: any number of top nodes (parent -1).
    Subtree,
}

/// A node's string, `None` when null or empty.
///
/// # Safety
/// `p` is null or a NUL-terminated string valid for `'a`.
#[must_use]
pub unsafe fn text<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        return None;
    }
    // SAFETY: the caller's contract; validation checked it is UTF-8.
    let s = unsafe { CStr::from_ptr(p) }.to_str().ok()?;
    (!s.is_empty()).then_some(s)
}

/// A state set without its ability.
const fn lacks(f: u32, state: u32, ability: u32) -> bool {
    f & state != 0 && f & ability == 0
}

const fn flags_valid(f: u32) -> bool {
    use flags::{
        CHECKABLE, CHECKED, EXPANDABLE, EXPANDED, FLAGS_ALL, FOCUSABLE, FOCUSED, LIVE_ASSERTIVE, LIVE_POLITE, MIXED,
        SELECTABLE, SELECTED,
    };
    f & !FLAGS_ALL == 0
        && !lacks(f, CHECKED | MIXED, CHECKABLE)
        && f & (CHECKED | MIXED) != (CHECKED | MIXED)
        && !lacks(f, EXPANDED, EXPANDABLE)
        && !lacks(f, SELECTED, SELECTABLE)
        && !lacks(f, FOCUSED, FOCUSABLE)
        && f & (LIVE_POLITE | LIVE_ASSERTIVE) != (LIVE_POLITE | LIVE_ASSERTIVE)
}

/// Check a flat tree at the boundary; everything past it relies on these rules.
///
/// # Safety
/// The node strings are null or NUL-terminated.
pub unsafe fn validate(nodes: &[Node], shape: Shape) -> Result<(), Invalid> {
    let mut ids: Vec<u64> = nodes.iter().map(|n| n.id).collect();
    ids.sort_unstable();
    if ids.first() == Some(&0) || ids.windows(2).any(|w| w[0] == w[1]) {
        return Err(Invalid::Id);
    }
    for (i, n) in nodes.iter().enumerate() {
        let parent_ok = match shape {
            Shape::Tree => (i == 0) == (n.parent == -1) && (n.parent < 0 || (n.parent as usize) < i),
            Shape::Subtree => n.parent == -1 || (n.parent >= 0 && (n.parent as usize) < i),
        };
        if !parent_ok {
            return Err(Invalid::Parent);
        }
        if Role::from_raw(n.role).is_none() {
            return Err(Invalid::Role);
        }
        if !flags_valid(n.flags) {
            return Err(Invalid::Flags);
        }
        for p in [n.name, n.description, n.value] {
            // SAFETY: the caller's contract.
            if !p.is_null() && unsafe { CStr::from_ptr(p) }.to_str().is_err() {
                return Err(Invalid::Utf8);
            }
        }
        if shape == Shape::Tree && !n.widget.is_null() {
            return Err(Invalid::Widget);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BUI_kit.h's `BK_ROLE_*`, `BK_A11Y_*` flags and actions are the ones mirrored here.
    #[test]
    fn header_agrees() {
        let h = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../kit/include/BUI_kit.h"))
            .expect("kit/include/BUI_kit.h");
        let roles: Vec<&str> = h
            .lines()
            .filter_map(|l| l.trim().strip_prefix("BK_ROLE_"))
            .map(|l| l.split([',', ' ']).next().unwrap_or(""))
            .filter(|n| *n != "COUNT")
            .collect();
        let ours: Vec<&str> = Role::ALL.iter().map(|r| r.c_name()).collect();
        assert_eq!(roles, ours, "BK_ROLE_* in BUI_kit.h and bui_a11y::Role differ");
        for (name, value) in [
            ("FOCUSABLE", flags::FOCUSABLE),
            ("DISABLED", flags::DISABLED),
            ("CHECKED", flags::CHECKED),
            ("CLIPS", flags::CLIPS),
            ("DEFAULT", flags::DEFAULT),
        ] {
            let shift = value.trailing_zeros();
            assert!(h.contains(&format!("BK_A11Y_{name} = 1 << {shift},")), "BK_A11Y_{name}");
        }
        assert!(h.contains("BK_A11Y_FLAGS_ALL = (1 << 25) - 1"));
        assert!(h.contains("BK_A11Y_CLICK = 0,") && h.contains("BK_A11Y_CONTEXT_MENU_SHOW,\n  BK_A11Y_ACTION_COUNT"));
    }

    fn node(id: u64, parent: i32, role: Role) -> Node {
        Node { id, parent, role: role as u16, ..Node::default() }
    }

    #[test]
    fn validates_at_the_boundary() {
        let ok = [node(1, -1, Role::Window), node(2, 0, Role::Button), node(3, 0, Role::Label)];
        // SAFETY: no strings.
        unsafe {
            assert_eq!(validate(&ok, Shape::Tree), Ok(()));
            assert_eq!(validate(&[node(1, -1, Role::Window), node(1, 0, Role::Button)], Shape::Tree), Err(Invalid::Id));
            assert_eq!(validate(&[node(0, -1, Role::Window)], Shape::Tree), Err(Invalid::Id));
            assert_eq!(validate(&[node(1, -1, Role::Window), node(2, 1, Role::Button)], Shape::Tree), Err(Invalid::Parent));
            assert_eq!(validate(&[node(1, -1, Role::Window), node(2, -1, Role::Button)], Shape::Tree), Err(Invalid::Parent));
            assert_eq!(validate(&[node(1, -1, Role::Label), node(2, -1, Role::Button)], Shape::Subtree), Ok(()));
            let mut bad = node(1, -1, Role::Window);
            bad.role = Role::ALL.len() as u16;
            assert_eq!(validate(&[bad], Shape::Tree), Err(Invalid::Role));
            for f in [flags::CHECKED, flags::CHECKABLE | flags::CHECKED | flags::MIXED, flags::EXPANDED, 1 << 30,
                      flags::LIVE_POLITE | flags::LIVE_ASSERTIVE, flags::FOCUSED] {
                let mut n = node(1, -1, Role::Window);
                n.flags = f;
                assert_eq!(validate(&[n], Shape::Tree), Err(Invalid::Flags), "{f:#x}");
            }
            let mut n = node(1, -1, Role::Window);
            n.name = c"\xff\xfe".as_ptr();
            assert_eq!(validate(&[n], Shape::Tree), Err(Invalid::Utf8));
            let mut w = node(1, -1, Role::Window);
            w.widget = std::ptr::dangling_mut();
            assert_eq!(validate(&[w], Shape::Tree), Err(Invalid::Widget));
            assert_eq!(validate(&[w], Shape::Subtree), Ok(()));
        }
    }
}
