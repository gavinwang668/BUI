# BUI demos

Small demos built on BUI's Rust API (`blenderui`). Source only, a few KB each. `demos/` is its own
cargo workspace, so it never touches `bindings/rust`'s build.

| Demo | Run | What BUI does in it |
|---|---|---|
| Shader components | `cargo run --release --bin shaders` | three `ShaderEffect` canvases: plasma button (hover), aurora card (cursor), ripple field (click) |
| Glass UI | `cargo run --release --bin glass` | a shader background under CSS glass: `backdrop-filter: blur() saturate()`, Blender widgets inside |
| CI pipeline | `cargo run --release --bin pipeline` | a GitHub Actions workflow as a Blender node graph, painted with `Painter`; run it, make the test job fail |
| Motion editor | `cargo run --release --bin editor` | After Effects-style layout: shader layer + painted layers, keyframes, timeline, effect controls that key at the playhead |
| Gambit | `cargo run --release --bin gambit` | a noir chess collage raymarched on the GPU: a half-resolution scene pass into a `RenderTarget`, then a full-resolution halftone composite. No UI: click anywhere and the queen slams down. `-- --frames DIR` renders frames off-screen and times the compile and both passes |
| Windows Failed | `cargo run --release --bin failed` | a Y2K blue poster wall (shader). No UI: the first click is a cascade of Win9x error dialogs (CSS keyframes, a shake, a glitch), each later one a single warning; every ✕ and OK closes its warning and opens another, and kernel panics pop up at random. `-- --storyboard DIR` clicks through it headlessly |
| Velvet Menu | `cargo run --release --bin velvet` | a Persona-style main menu for a tech app: an underwater shader, a tilted heavy-italic menu where the selection snaps forward on a red slash (hover or ↑/↓), Enter / click opens a sub-menu that follows the arrows, C / Esc closes. Laid out in `rem` on a 1280 x 720 canvas sized to the window, so it stays sharp. `-- --storyboard DIR` drives it headlessly |
| Phantom | `cargo run --release --bin phantom` | a Persona 5-style menu for the same app: a shader scene (red room, spinning starburst, star, ink slash) under ransom-note type, every letter its own font, tilt and box; the selection's letters pop onto coloured blocks, Enter slams in the sub-menu card |
| Morning | `cargo run --release --bin morning` | a pinch folds a full-screen page into its card in a stack (cloned from `ref/testanim.mp4`): the page washes out, shrinks onto its card and goes dark while the stack zooms up behind it, its three cards fanning in. Pinch on a trackpad or touch screen to scrub it (letting go finishes the way you were going); Space folds, Escape opens, a click does either |

`src/menu.rs` (the commands and the selection state) and `src/canvas.rs` (a design canvas that fits the window crisply) are shared by the two menus. `src/lib.rs` is shared by all of them: the window, the shader + software fallback (`Fx`, with an optional pre-pass into a render target), the clock, `--screenshot out.png` (off-screen, software), scripted headless runs and GPU stills.
Without a GPU, each shader draws its flat fallback colour.

Build output goes to `demos/target/` (ignored). Delete it to reclaim the space.
