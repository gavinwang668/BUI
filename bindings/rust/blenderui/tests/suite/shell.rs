//! The desktop shell from Rust (`blenderui::shell`): a tray icon with a nested menu, its icon and
//! tooltip, a notification, the recent documents and the checks at the boundary. Skips the parts
//! this build or system lacks (`features()`), and only those; the events are the C test's
//! (tests/shell_test.c), which posts the system's messages to the shell's window.

use blenderui::shell::{self, MenuItem, Tray};

#[test]
fn tray_menu_notification_and_recent_files() {
    let features = shell::features();
    if !cfg!(feature = "shell") {
        assert_eq!(features, shell::ShellFeatures::default(), "without the feature, nothing");
        return;
    }
    if features.tray() {
        let tray = Tray::new(Some("BLENDER"), "blenderui shell test").expect("a tray where features() has one");
        assert!(tray.set_tooltip("blenderui shell test: tooltip"));
        let png = format!("{}/../../../tests/images/quad.png", env!("CARGO_MANIFEST_DIR"));
        assert!(tray.set_icon(Some(&png)), "an image file");
        assert!(!tray.set_icon(Some("no-such-image.png")), "not an image: refused, the old icon stays");
        assert!(tray.set_icon(None), "the app's own icon");
        let more = [MenuItem::action(10, "A"), MenuItem::action(11, "B").disabled()];
        assert!(tray.set_menu(&[
            MenuItem::action(1, "Open"),
            MenuItem::Separator,
            MenuItem::check(2, "Mute", true),
            MenuItem::submenu("More", &more),
            MenuItem::action(3, "Quit\0ignored after a NUL"),
        ]));
        assert!(tray.set_menu(&[]), "an empty menu removes it");
        let other = Tray::new(None, "a second one").expect("a second tray");
        assert_ne!(tray.id(), other.id());
    } else {
        eprintln!("no tray on this system: tray part skipped");
    }
    if features.notifications() {
        let a = shell::notify("blenderui", "A notification from the Rust test", None).expect("shown");
        let b = shell::notify("blenderui", "With an icon", Some("BLENDER")).expect("shown");
        assert!(b.0 > a.0, "ids grow");
    }
    assert!(shell::notify("", "", None).is_none(), "nothing to show: refused");
    assert!(!shell::add_recent_document("no/such/file.txt"), "a file that isn't there: refused");
    assert!(!shell::add_recent_document(""), "no path: refused");
}
