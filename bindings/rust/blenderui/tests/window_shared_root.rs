//! A root view that two windows show lives until the last of them is freed: closing the first leaves
//! the second one drawing it, closing the second releases it.

use blenderui::prelude::*;

struct Page;

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child("shared")
    }
}

#[test]
fn a_shared_root_outlives_the_first_window_to_close() {
    Application::headless().run(|cx| {
        let shared = cx.new(|_| Page);
        let first = cx.open_window(WindowOptions::default(), move |_, _| shared).unwrap();
        let second = cx.open_window(WindowOptions::default(), move |_, _| shared).unwrap();
        cx.draw(first.id);
        cx.draw(second.id);

        cx.close_window(first.id);
        cx.run_pending();
        assert!(shared.downgrade().upgrade_in(cx).is_some(), "the second window still shows the root");
        let (w, h, _) = cx.draw(second.id); // would panic on a released root
        assert!(w > 0 && h > 0);

        cx.close_window(second.id);
        cx.run_pending();
        assert!(shared.downgrade().upgrade_in(cx).is_none(), "the last window to close releases the root");
    });
}
