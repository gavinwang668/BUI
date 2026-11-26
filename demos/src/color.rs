//! Colours as the page writes them: `#rrggbb`, from and to the `u32`s the demos keep (a
//! `ColorSwatch` reports its colour as `#rrggbb`).

/// `0xrrggbb` as `#rrggbb`.
#[must_use]
pub fn hex(c: u32) -> String {
    format!("#{c:06x}")
}

/// `#rrggbb` (or `rrggbb`; any digits after the sixth, an alpha, are left out) as `0xrrggbb`.
#[must_use]
pub fn parse_hex(s: &str) -> Option<u32> {
    u32::from_str_radix(s.trim_start_matches('#').get(..6)?, 16).ok()
}
