# BUI on Android

BUI runs on a phone as one `NativeActivity` (no Java, no Gradle) with the software raster, touch and the
soft keyboard. **The window is the kit's:** `kit/intern/kit_window_android.c` implements `BK_window_*`
the way `kit_window_win32.c` and `kit_window_x11.c` do, so an app opens a window and runs the loop
exactly as on a desktop. C, Lua and Rust apps are the same code they would be on Windows or Linux.

Nothing changes for other platforms: the Android code lives in `platform/android`, in the kit's
Android-only branches, and in `if(ANDROID)` blocks of CMake.

```
python platform/android/build.py                       # the C demo: arm64-v8a + x86_64 -> build-android/bui.apk
python platform/android/build.py --abi arm64-v8a       # a phone only
python platform/android/build.py --install             # adb install -r, then start it
python platform/android/build.py --lua examples/android        # a Lua app (the folder's *.lua are assets)
python platform/android/build.py --rust platform/android/rust  # a Rust app (a staticlib crate)
python platform/android/build.py --js app.jsx                  # a JavaScript app: React, Vue or Svelte on QuickJS
python platform/android/build.py --imgui [app.cpp]             # a Dear ImGui app (C++)
python platform/android/build.py --rust app/android --label MyApp --package com.me.app --permission INTERNET
```

`--label` names the app on the phone (and the APK file), `--package` is its id (default `org.bui.app`),
and each `--permission` adds one (`INTERNET` for an app that goes online; a bare name means
`android.permission.NAME`).

Needs the Android SDK (`ANDROID_HOME`; build-tools, platform 34), an NDK (the newest in the SDK, or
`ANDROID_NDK_HOME`), CMake, a JDK for `keytool`, and for Rust `cargo` with the Android targets
(`rustup target add aarch64-linux-android x86_64-linux-android`). The APK is signed with
`~/.android/debug.keystore`.

## Writing an app

| Language | You write | Run it |
| --- | --- | --- |
| **Rust** | an ordinary `blenderui` app in a `staticlib` crate with `blenderui::android_main!(run);`, where `run` is the `fn()` a desktop `main` would call (`platform/android/rust`) | `--rust CRATE` |
| **Lua** | an ordinary BUI Lua script, `assets/main.lua`, ending in `bui.open{root=...}` or `bui.Window(...)` + `bui.run()`; `require` finds your other `.lua` files and `bui.lua` in the APK | `--lua DIR` |
| **JavaScript** | an ordinary `@blender-ui/native`, React, Vue or Svelte app (`.js`, `.jsx`, `.vue`, `.svelte`); it is bundled into one script (`bui bundle`, needs Bun) and runs on QuickJS (`bindings/rust/blenderui-qjs`, the engine `bui build` embeds), as `assets/main.js`; `console.log` goes to logcat (tag `bui-js`) | `--js ENTRY` |
| **Dear ImGui** | an ordinary ImGui frame function in a C++ file with `bui_app_main` (`ImGui_ImplBUI_CreateApp` / `RunApp`; `platform/android/imgui_app.cpp`); a drag is ImGui's (sliders, window moves), since the adapter puts its node in `BK_TOUCH_MODE_POINTER` | `--imgui [FILE]` |
| **C** | `void bui_app_main(void)`: build nodes, `BK_context_new`, `BK_window_new`, `BK_window_run` (`platform/android/demo.c`) | the default |

Python, Qt, tkinter and D are not available on Android: each needs its
runtime on the device.

The Rust demo (`platform/android/rust`) has three tabs: the widgets, a list of **10 000 000 rows** and a
chat of **a million messages** (`blenderui::vlist`: only the visible rows exist, the row text is computed
when it is fetched, a frame costs O(visible rows)). The emulator runs it at about 75 MB.

For Rust the staticlib carries its own copy of the kit (`blenderui-sys` builds it for the target), so
the app crate sets the kit's features itself (see its `Cargo.toml`: no GPU, ImGui or extra image
formats). The system libraries the staticlib needs (rustc's `native-static-libs`: `aaudio` for a
crate that plays sound, ...) are linked into the shared library for it.

`android_main!` writes the unmangled entry point (so the app has no `unsafe`) and, before the app
runs:

- gives the activity's JNI handles to `ndk-context`, which crates that call into Java read (cpal
  lists sound devices through it; without it they panic);
- sends stdout and stderr to logcat, panics included: `adb logcat -s bui`;
- and the kit has pointed `HOME` at the app's files folder and `XDG_CACHE_HOME` / `TMPDIR` at its
  cache folder, so `dirs::cache_dir()`, `dirs::home_dir()` and `std::env::temp_dir()` are folders the
  app may write.

A real app is SpotiTube-sized: a library crate shared with the desktop `main`, plus a small
`staticlib` crate whose whole source is the `android_main!` line.

## Behavior

- **Damage driven.** Nothing is drawn and the thread sleeps in `ALooper` until a touch, key, timer, wake
  or frame request. Only the damaged rectangles are converted (B,G,R words to the surface's R,G,B bytes)
  and locked. Idle cost: 0 CPU ticks in 10 s on the emulator, for the C and the Rust app.
- **Frame rate.** The app runs at the display's fastest refresh rate: the host names the fastest mode
  at the current size on the window (`preferredDisplayModeId` / `preferredRefreshRate`, from the
  activity's `onWindowFocusChanged`) and votes with `ANativeWindow_setFrameRate`, so a 90 Hz panel is
  not held to 60 Hz. `-DBUI_ANDROID_FRAME_LOG` logs frames and CPU time per second to logcat (tag `BUI`).
- **Scale.** A CSS pixel is a dp, as in Chrome on Android: the kit's scale is the screen density
  (`dpi / 160`), so Rust layout and Blender's 20-unit widgets come out at a phone's size.
- **Touch.** The host forwards every pointer of a motion event (batched history first) to the kit's
  touch model (`BK_touch`), the same one every window host uses: see [touch.md](touch.md). A tap
  is a click, a drag that starts on a widget that drags (sliders, number fields, text, scrollbars)
  drags it, and anywhere else it scrolls, following the finger exactly and flinging on along
  Android's `OverScroller` curve when it lifts; a touch stops the fling. A long press opens the
  context menu, and two fingers pan, pinch and turn. A touch in the 20 dp strip at the right edge
  of a custom or canvas node is the app's press and drag (a list's scrollbar thumb), so a huge list
  can be dragged to any place. The old `BK_window_wm(win, "touch_drag", 1, 0)` key is gone: a custom
  node that wants a finger's drag (a paint app) calls `BK_custom_set_touch(node,
  BK_TOUCH_MODE_POINTER)`, or `BK_TOUCH_MODE_RAW` for every contact.
- **Back.** The Back key is the key `Escape`. It closes a popup or leaves a text field first; a Rust
  key listener that acts on it calls `app.prevent_default()` (C bindings: `BK_key_taken`), and only a
  Back nothing took finishes the activity. An app with screens of its own walks back through them
  this way.
- **Keyboard.** The soft keyboard opens while a text field or area is being edited (through the input
  method manager: the activity's view is not a text editor, so `ANativeActivity_showSoftInput` is
  ignored by current Android). Keys arrive as key events (`KeyEvent.getUnicodeChar`), so typing,
  Backspace, arrows and Back (Escape) work. Predictive and composing IMEs (`BK_ime_event`) need an
  `InputConnection` and are not wired up.
- **System bars.** The UI is drawn inside the activity's content rectangle; the soft keyboard resizes it
  (`adjustResize`).
- **Window.** One window, the surface. `BK_window_new` waits for the surface so the app builds its UI at
  the real size and scale. `BK_window_set_size` and most `BK_window_wm` keys do nothing;
  `BK_window_wm_get` answers `scale`, `screen` and `pointer`. `BK_window_timer` returns -1 when the
  window's 16 timers are all in use, as on the other hosts.
- **Lifecycle.** One activity (`launchMode="singleTask"`): launching the app while it runs brings it
  back. Back finishes it and `bui_app_main` returns; the next launch may reuse the process, and runs
  `bui_app_main` again from a fresh start (`BK_android_attach` resets the host). Once `android_main`
  returns (`BK_android_detach`), a `BK_window_wake` from another thread does nothing. A Lua script that
  fails shows its error on screen, also after it opened its window (closing the Lua state frees it).

## Fonts

Inter is left out of the build (`BUI_NO_INTER`, 0.8 MB). The shell registers the system's fonts
(`platform/android/fonts.c`: Roboto and Droid Sans Mono from `/system/fonts` as the families `sans` and
`mono`). Characters Roboto lacks (Hangul, kana, Han, Thai, Indic, symbols) come from the phone's Noto
fonts in `/system/fonts`, by the per-glyph fallback every platform has. To use your own, call `BUI_text_add_font(family, flags, data, len)` with the font's bytes
(from `BK_android_assets()` or a file) and select it with `BK_set_font(family)`, as on any platform.

## Size

`-O2` with link-time optimization (C and Lua apps; Rust does its own), `-fvisibility=hidden`,
`--gc-sections`, stripped, GPU renderers, Dear ImGui, the file browser and every image format but PNG and
SVG off (`features.json`). arm64-v8a: the C demo is 2.5 MB (APK about 1.1 MB), with Lua 3.2 MB (APK 1.4
MB), with Rust 4.3 MB (APK 2.0 MB; blenderui's layout, CSS and Tailwind engines). The APK deflates the
library and the installer extracts it, so `zipalign` aligns the stored entries to 4 bytes only (no `-P`
page alignment, which build-tools 34 lacks and a deflated library does not need). `-Os` saved only 4%,
so speed was kept. About 0.45 MB is the icon
set.

## Tested, and not

Run on an x86_64 emulator (API 33): the C demo, the Lua app (a button callback updates its label), the React, Vue and Svelte counters (QuickJS: clicks, the keyboard in an input, a slider) and the
Rust app (a button, a text input, scrolling with the scrollbar). The arm64-v8a builds of all three
compile and link. On a phone, the refresh rate was checked on a 90 Hz panel (60 Hz before the fix,
90 Hz after), and SpotiTube's Android build is a Rust app on `android_main!`.

Not done: a GPU renderer (OpenGL ES would reuse `gpu_gl.c`, untested),
IME composition, a clipboard beyond the app's own, cutout insets beyond the content rectangle, and
touch text selection in Rust pages (a drag scrolls).
