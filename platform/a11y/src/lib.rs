//! # bui-a11y
//!
//! Screen-reader accessibility for the BUI kit, the one place AccessKit is used.
//!
//! The kit (kit/intern/a11y) builds each window's accessibility tree as a flat array of
//! [`Node`]s (`BKA11yNode` in `BUI_kit.h`): its own widgets, and the subtrees hosts attach (the
//! Rust HTML engine's elements). This crate
//!
//! * checks such arrays at the boundary ([`validate`]),
//! * turns them into AccessKit tree updates, the whole tree once and then only the nodes that
//!   changed ([`Tree`]),
//! * runs the platform adapter of each native window ([`Adapter`]): UI Automation on Windows
//!   (accesskit_windows). It starts when a client first asks for the tree (`WM_GETOBJECT`), so a
//!   window nobody inspects builds no tree. Requests from screen readers are queued for the
//!   window's thread, which performs them with `BK_a11y_action`.
//!
//! C programs use it through the ABI in `include/bui_a11y.h` (built as a static library by the
//! kit's CMake when `BUI_A11Y` is on); Rust programs link it through `blenderui-sys`.

pub mod adapter;
mod ffi;
pub mod node;
pub mod tree;

pub use adapter::{Adapter, Host};
pub use node::{Action, Invalid, Node, Request, Role, Shape, flags, validate};
pub use tree::Tree;
