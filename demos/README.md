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
| HUD Shell | `cargo run --release --bin hud` | a heads-up-display desktop: a taskbar whose slots launch, focus and minimise apps; a launcher, right-click choice lists, slide-in notifications, typed status lines, bio-monitor bars, and a GPU minimap radar in the clock box over a scanline shader that tears on every launch. Keys: S, 1-7, Tab, X, W, N, Esc |
| Breeze | `cargo run --release --bin aero` | a Frutiger Aero music app: a shader sky (clouds, sun flare, droplets, swaying grass) under CSS glass panels, and it plays: click a track, drag the scrubber and the volume fader, like, shuffle, skip; play / pause cross-fade (scale + blur); shader cover art. Space, ← / →, ↑ / ↓, N / P |
| Lagoon | `cargo run --release --bin amp` | a skinned 2000s MP3 player in three docked windows: a `Painter` spectrum analyzer (click: oscilloscope) that follows the EQ, a stepped marquee, ten draggable EQ bands with a live curve and presets that slide in, a playlist (select, double-click, add / remove / shuffle / sort); EQ and PL fold their windows |
| Dayline | `cargo run --release --bin tasks` | a dark desktop planner with a white task sheet: checking draws the tick, strikes the title and folds the row out (Undo toast), groups fold under their chevrons, the selection pill glides, the detail pane staggers in with a streak, a progress ring and subtasks you type in place; search filters as you type, list or board. ↑ / ↓, Space, Enter, Esc, Ctrl K |
| Clock | `cargo run --release --bin clock` | a soft-UI clock app on two phones in two looks that cross-fade (CSS-variable shadows, painted hands and arcs): a live analog face, alarm switches that spring across, a stopwatch whose 60-tick ring fills with laps dropping in, and a glossy timer knob you drag or scroll whose glowing arc drains as it counts down. The gear or T switches the look |
| Headphones | `cargo run --release --bin headphones` | a split light / dark Bluetooth headphone controller: the control surface is laid out twice and clipped either side of a seam you can drag through it. Draggable / scrollable LEFT / RIGHT knobs (LINK keeps their offset), GAIN / MIX faders, a LPASS / HPASS pair, ANC / AMBIENT, SPATIAL, SYNC, presets that glide every value, power, a live stereo meter |
| Orbit Messenger | `cargo run --release --bin y2k` | a Y2K chat client over a liquid-chrome metaball shader (iridescent film, one blob follows the pointer): aqua glass panels, orb avatars, sliding pill tabs; send with Enter for a starburst and a typed reply, BUZZ! shakes the window, discs download in Files, and a mood fader retints the whole chrome world |
| Dreamlog | `cargo run --release --bin dream` | a Dreamcore dream journal over one GPU shader (a pastel field with a rainbow and a checkerboard floor, endless pool rooms, a night field) through a worn VHS tape: eyes in the clouds follow the pointer, a dream-depth dial makes it stranger, the door crosses into the next place through a bloom, and waking up never works. ↑ / ↓, D, W, Space |
| Intro | `cargo run --release --bin intro` | a portfolio that opens as a terminal on a liquid-metal shader frame (it types its command; Return runs it, Esc skips) and hands over to the hero, stat counters counting up; then a page that scrolls with inertia (wheel, keys, drag-fling) into a glass carousel that springs between products: drag / swipe, ← / →, Shift + wheel, buttons, auto-advance every 3.5 s unless hovered |
| ASCII Field | `cargo run --release --bin ascii` | a dark portfolio over one GPU shader: a drifting FBM scene drawn as 5 × 5 bitmap glyphs per cell, lit into code by a halo round the pointer that eases in and out; cards with a pointer-following glow that lift, and a `Painter` glyph field that morphs and brightens near the pointer |

## Blender UI apps

Everyday apps built only from what Blender's interface has, for Blender users who want to know what it can do outside Blender: Blender's screen of areas (split, join, maximize them, each with its editor-type menu, header, toolbar, sidebar and footer), Blender's own widgets (number fields you drag or type math into, sliders, menus, the colour picker, checkboxes, radios, text fields), panels and Blender's theme. Everything in them works.

| App | Run | What it shows |
|---|---|---|
| BUI Mail | `cargo run --release --bin bui_mail` | an email client: folders, list and reader as areas you split / maximize; search that filters as you type; account and sort menus; a checkbox per row with Shift / Ctrl ranges; drag a message onto a folder; conversations as panels; label colours from Blender's colour picker; rules with number fields; reply / forward with a signature menu and uploading attachments; a send you can undo; every action undoable. J / K, E, R, S, Ctrl+Z |
| BUI Chat | `cargo run --release --bin bui_chat` | a community chat: the server rail is a toolbar region, threads open in the sidebar region, split chat areas each keep their own channel; reactions, replies, threads, a composer answered by invented members, a live member filter; a voice channel with `Painter` level meters and a volume slider each; settings (status menu, name colour, accent radios, a UI-scale slider) that apply at once |
| BUI Files | `cargo run --release --bin bui_files` | a file explorer on the real file system, read-only: places and a folder tree, list or thumbnail grid (real PNG thumbnails), image and text preview, sort and live filter, box select, type-ahead; rename, new folder, copy with progress, drag-move, delete and undo, all inside a sandbox in the temp folder; Blender's own File Browser one click away |
| BUI Timer | `cargo run --release --bin bui_timer` | clocks: world clock faces, alarms (number fields you scrub to set the time), several timers with rings, stopwatch laps with best / worst, Pomodoro with a focus mode that maximizes its area, workspaces that remember their layout, an accent colour that recolours Blender's widgets, chimes, tray and taskbar progress |
| BUI Notes | `cargo run --release --bin bui_notes` | a block wiki: a page tree you drag to reorder and nest, a slash menu, Markdown shortcuts, to-dos, toggles, lists, quotes, callouts and code; a task database as a table (status menus, hour fields) and a kanban board you drag cards across; search; two pages side by side; undo / redo |
| BUI Writer | `cargo run --release --bin bui_writer` | a word processor: a ribbon of Blender widgets (style and font menus, size field, colour pickers, B / I / U), A4 pages with a ruler you drag and real pagination, rich text with a caret and selection, tables, lists, find & replace, spelling, comments, a navigator of headings, zoom, Markdown export |
| BUI Sheet | `cargo run --release --bin bui_sheet` | a spreadsheet: a formula engine (cell and range refs across sheets, `SUM AVERAGE MIN MAX COUNT IF ROUND` ..., cycle and error detection), number formats, fill handle, copy / paste shifting refs, column resize and sort, sheet tabs, a live chart area of the selection, undo / redo; only the visible cells are built |
| BUI Calendar | `cargo run --release --bin bui_calendar` | a calendar: day, week, month and agenda views; drag on empty time to create, drag to move, drag the edge to resize; overlapping events in columns; weekly repeats, all-day events, time zones, calendars you show and colour; an event editor of Blender fields (length takes math); a now line; undo / redo |

Every app takes `-- --storyboard DIR`, which drives all of it headlessly and saves frames.

### What Blender's UI can and can't do (from Rust, today)

What building these found. "With a workaround" says how the apps do it.

| | |
|---|---|
| **As it is** | Blender's areas: split, join, dock, maximize, the editor-type menu, header / toolbar / sidebar / footer regions, saved layouts (workspaces) |
| | Blender's widgets with Blender's behaviour: drag a number to change it, double-click to type (math such as `25*60` works), Ctrl+Z, Esc; sliders, menus, the colour picker, checkboxes, radios, progress bars; `.precision(n)` for the decimals shown |
| | Text fields that report every keystroke (`attr("live", "")`, as Blender's search fields) or the confirmed edit |
| | Keyboard shortcuts: `window.text_field_focused()` keeps single-key shortcuts quiet while the user types; `cx.prevent_default()` takes a key from Blender's own handling (its Ctrl+Z widget undo, a hovered widget's keys) |
| | Panels (`details` / `summary`), Blender's theme and theme colours, `Painter` drawing, GPU shaders, CSS layout, effects and animation, springs |
| | Desktop: notifications, a tray icon, taskbar progress, Blender's File Browser |
| **With a workaround** | Drag and drop between elements: an element's position is known while rendering, so the apps keep the drop targets from the last render |
| | Rich text: Blender's text fields hold plain text; the writer keeps its own text model and draws styled runs |
| | Clearing a field after a send: a value is pushed only when it changes, so the field gets a new id |
| | Long lists and grids: build the visible rows only (the `blenderui-vlist` / `blenderui-table` crates do this) |
| | Toggle buttons: Blender's `Button` has no pressed state, so toggles are styled boxes |
| | Tall headers or footers: Blender's header regions keep Blender's height; taller content goes in the main view |
| **Not possible yet** | Popup and context menus other than a `Dropdown`, and hover tooltips |
| | Anything drawn over a Blender widget: widgets are always on top, so a dialog can't cover a field |
| | The node editor |
| | Starting a text edit from code, and text layout queries (the caret by line) |
| | Blender's widgets follow Blender's UI scale, not CSS sizes |

Shared by the demos (`src/`):

- `lib.rs`: the window, the shader + software fallback (`Fx`, with an optional pre-pass into a render target), the clock (`now`, virtual in a headless run so storyboards are deterministic), `--screenshot out.png` (off-screen, software), `--size WxH`, scripted headless runs and GPU stills.
- `story.rs`: `--storyboard DIR` runs, which click, type, hover and press keys through a demo and save frames along the virtual clock.
- `canvas.rs`: a design canvas that fits the window crisply (layout in `rem`), and placing on it.
- `control.rs`: knobs, faders and scrubbers (`Dial`, `draggable`: drag with pointer capture, or the wheel), and dial arcs with a glow.
- `pen.rs`: a `Painter` that draws in a design box's own units (dials, icons), and colour mixing.
- `ease.rs`: the curves the hand-driven motion is made of. `text.rs`: a typed-into text draft. `color.rs`: `#rrggbb` to and from numbers.
- `lib.rs` also has `act` (a listener from a view's weak handle, for a screen's editor factories) and `area_id` (element ids for an editor shown in two areas).
- `menu.rs`: the commands and the selection state of the two menus (Velvet, Phantom).

Every demo with interaction takes `-- --storyboard DIR`, and every shader demo `-- --frames DIR` (GPU stills).
Without a GPU, each shader draws its flat fallback colour.

Build output goes to `demos/target/` (ignored). Delete it to reclaim the space.
