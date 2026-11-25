//! What the menu demos (Velvet, Phantom) share: the tech app's commands and their sub-menus, and
//! the selection state behind any menu of them. Each demo only draws it its own way.

/// One command: its label, what the hint says, and its sub-menu (label, value).
pub struct Entry {
    pub name: &'static str,
    pub hint: &'static str,
    pub rows: [(&'static str, &'static str); 4],
}

pub const ENTRIES: [Entry; 9] = [
    Entry { name: "DASHBOARD", hint: "Open the Dashboard",
            rows: [("Requests / min", "1,284"), ("Error rate", "0.02%"), ("p95 latency", "84 ms"), ("Active users", "3,912")] },
    Entry { name: "PROJECTS", hint: "Browse Projects",
            rows: [("bui-core", "main · 2m ago"), ("website", "preview · 1h"), ("bindings-rust", "main · 3h"), ("design-system", "draft")] },
    Entry { name: "DEPLOY", hint: "Ship a Deploy",
            rows: [("Production", "v2.14.0 · live"), ("Staging", "v2.15.0-rc1"), ("Preview #482", "building…"), ("Rollback to", "v2.13.2")] },
    Entry { name: "ANALYTICS", hint: "Read Analytics",
            rows: [("Visitors", "48.2k"), ("Conversion", "3.4%"), ("Bounce rate", "41%"), ("Avg. session", "4m 12s")] },
    Entry { name: "TEAM", hint: "Manage your Team",
            rows: [("Ada", "Owner"), ("Linus", "Admin"), ("Grace", "Developer"), ("Alan", "Viewer")] },
    Entry { name: "INBOX", hint: "Check your Inbox",
            rows: [("Build failed on main", "now"), ("Review requested", "12m"), ("Invoice paid", "1h"), ("New sign-in", "3h")] },
    Entry { name: "INTEGRATIONS", hint: "Connect Integrations",
            rows: [("GitHub", "Connected"), ("Slack", "Connected"), ("Sentry", "Paused"), ("Stripe", "Connected")] },
    Entry { name: "BILLING", hint: "Review Billing",
            rows: [("Plan", "Pro"), ("This month", "$412.80"), ("Seats", "8 of 10"), ("Next invoice", "Jun 1")] },
    Entry { name: "SETTINGS", hint: "App Settings",
            rows: [("Profile", ""), ("Security", ""), ("API keys", ""), ("Sign out", "")] },
];

/// The selection, the open sub-menu and the one on its way out. Every method answers whether
/// anything changed (so the view re-renders only then).
#[derive(Default)]
pub struct MenuState {
    /// The entry under the pointer or the keys.
    pub sel: usize,
    /// Bumped on every selection: keys what plays on a selection, so it plays again.
    pub picks: u32,
    pub open: Option<usize>,
    pub closing: Option<usize>,
    /// Confirms so far: keys what plays on a confirm.
    pub confirms: u32,
    /// When the last confirm was (`now()`).
    pub confirmed: Option<f32>,
}

impl MenuState {
    /// Moves the selection; an open sub-menu follows it.
    pub fn select(&mut self, i: usize) -> bool {
        if self.sel == i {
            return false;
        }
        self.sel = i;
        self.picks += 1;
        if self.open.is_some() {
            self.show();
        }
        true
    }

    fn show(&mut self) {
        if self.open != Some(self.sel) {
            self.closing = self.open.take();
            self.open = Some(self.sel);
        }
        self.confirms += 1;
    }

    /// Enter / click / Confirm: opens the selected entry's sub-menu, or plays the confirm again.
    pub fn confirm(&mut self) -> bool {
        self.show();
        self.confirmed = Some(crate::now());
        true
    }

    pub fn close(&mut self) -> bool {
        self.open.take().is_some_and(|i| {
            self.closing = Some(i);
            true
        })
    }

    /// ↑ / ↓ move, Enter / Space confirm, C / Escape close.
    pub fn key(&mut self, key: &str) -> bool {
        let n = ENTRIES.len();
        match key {
            "Up" => self.select((self.sel + n - 1) % n),
            "Down" => self.select((self.sel + 1) % n),
            "Return" | "KP_Enter" | "space" => self.confirm(),
            "c" | "C" | "Escape" => self.close(),
            _ => false,
        }
    }

    /// The hint for what is open, else for what is selected.
    #[must_use]
    pub fn hint(&self) -> &'static str {
        ENTRIES[self.open.unwrap_or(self.sel)].hint
    }
}
