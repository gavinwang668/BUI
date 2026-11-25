# Touch screens and trackpad gestures

A finger on a BUI window works as it does on a phone: a tap is a click, a drag scrolls and flings,
a long press opens the context menu, and two fingers pan, pinch and turn. Trackpad pinches and
turns reach the same places. The model lives in the kit (`kit/intern/kit_touch.c`), so it is the
same on every platform; a window host only forwards the contacts it gets (`BK_touch`) and the
trackpad's gestures (`BK_magnify`, `BK_rotate`).

The numbers and curves are Android's (`ViewConfiguration`, `VelocityTracker`, `OverScroller`), and
the gestures land where Blender's trackpad events do: GHOST's `GHOST_kTrackpadEventScroll`,
`GHOST_kTrackpadEventMagnify` and `GHOST_kTrackpadEventRotate` become the window manager's
`MOUSEPAN`, `MOUSEZOOM` and `MOUSEROTATE` (`view2d_ops.cc`: `view_pan`, `view_zoom`).

## What a finger does

| Contact | Result |
|---|---|
| One finger on a widget with a drag of its own (slider, number or text field, scrollbar, node editor, a `BK_TOUCH_MODE_POINTER` custom node, the scrollbar strip at a page's right edge) | The pointer at once: motion, press, drag, release, with `BK_FROM_TOUCH` in the mods |
| One finger anywhere else, lifted in place | A click (a tap); two taps are a double click. For fingers the double-click slop is one widget unit |
| Held still for half a second (`DEFAULT_LONG_PRESS_TIMEOUT`) | A right press: the context menu (`BK_context_menu`) |
| Moved past the touch slop (8 dp, `TOUCH_SLOP`) | Scrolls what is under where it went down, exactly as far as it went (`BK_scroll_px` with `BK_SCROLL_DRAG`), from just past the slop so the content does not jump (RecyclerView's way) |
| Lifted while moving | The content flings on at the finger's velocity along `OverScroller`'s curve (`BK_SCROLL_FLING`, then `BK_SCROLL_END`). `BK_animating` is non-zero meanwhile and each `BK_render` steps it |
| A finger that lands on a fling | Stops it and clicks nothing |
| A second finger | Cancels what the first one did, as `BK_cancel` (a dragged value goes back, nothing is clicked). The two are a gesture until they lift |
| Two fingers moving together | Scroll (`MOUSEPAN`) |
| Two fingers spreading / turning | Magnify / rotate about their centre (`MOUSEZOOM`, `MOUSEROTATE`, as `BK_magnify`, `BK_rotate`) |
| `BK_TOUCH_CANCEL` | The platform took the contact back (a system gesture, a lost capture): nothing it did is kept |

More rules:

- Hosts report fingers one at a time, so a two-finger update waits until both moved (or one moved
  twice). A pure pan does not wobble the spread and angle between them.
- After a gesture, no finger is the pointer again until all have lifted. A third finger is
  followed but unused; up to ten contacts are followed.
- While a menu is open or a tool runs (the eyedropper), every finger is the pointer.
- No hover stays behind a finger: the pointer leaves when the finger that was it lifts.
- `BK_cancel` and a window losing the focus cancel every contact that is down.
- A finger's drag never warps the system pointer (the mouse's endless number-field drag does).
- A scroll keeps to its axis; a diagonal drag scrolls both.
- In the node editor two fingers pan the view and a pinch (or a trackpad's) zooms about the
  centre. A link drag that is cancelled puts back a link it took off an input (`node_link_cancel`),
  and `Delete` / `X` do nothing in the middle of a link or value drag.

## C API (`kit/include/BUI_kit.h`)

```c
enum { BK_TOUCH_DOWN = 0, BK_TOUCH_MOVE, BK_TOUCH_UP, BK_TOUCH_CANCEL };
int BK_touch(BKContext *ctx, int id, int phase, float x, float y, double time_ms, int mods);
int BK_magnify(BKContext *ctx, int x, int y, float delta, int mods);   /* trackpad pinch */
int BK_rotate(BKContext *ctx, int x, int y, float degrees, int mods);  /* trackpad turn */
```

- `id` is the platform's id of the contact (unique while it is down).
- `x`, `y` are window pixels, as for `BK_motion`. A cancel's position is not used.
- `time_ms` is the event's own time in milliseconds on any monotonic clock; only differences
  count (the fling's velocity). `<= 0` means now.
- `mods` are `BK_SHIFT | BK_CTRL | BK_ALT`. Hosts never pass `BK_FROM_TOUCH`: the kit sets it on
  the pointer, scroll and gesture events it makes from contacts, so raw handlers and custom nodes
  can tell a finger from the mouse.

### Sign conventions

| Value | Meaning |
|---|---|
| `BK_magnify` `delta` | The scale change since the last one: 0.1 is 10% larger, -0.1 smaller (`NSEvent.magnification`'s units). A zoom is multiplied by `1 + delta` |
| `BK_rotate` `degrees` | Degrees since the last one, positive counter-clockwise (`NSEvent.rotation`) |
| A pan | The content follows the fingers: a finger moving up scrolls the content up |

### Forwarding contacts from your own host

```c
/* one contact of a touch screen event: its id, phase and window position */
static void on_touch(BKContext *ctx, int id, int kind, float x, float y, double ms, int mods)
{
  int phase = kind == MY_DOWN ? BK_TOUCH_DOWN :
              kind == MY_UP   ? BK_TOUCH_UP :
              kind == MY_MOVE ? BK_TOUCH_MOVE : BK_TOUCH_CANCEL;
  if (BK_touch(ctx, id, phase, x, y, ms, mods)) {
    request_redraw();
  }
}

/* a trackpad's pinch and turn about the pointer */
BK_magnify(ctx, px, py, scale_change, mods);
BK_rotate(ctx, px, py, degrees_ccw, mods);

/* every frame: render while the kit animates (a fling) */
while (BK_animating(ctx)) {
  BK_render(ctx, raster, w, h, 0);
  present();
}
```

Send a moved contact's batched history first, then its current place. Don't also send the mouse
messages a platform makes from a finger: the kit's model is the only one.

### Raw events

An `"events"` node gets `BK_EV_GESTURE` for a pinch or a turn over it: `ref` is
`"mods kind x y"` (`kind` `BK_GESTURE_MAGNIFY` or `BK_GESTURE_ROTATE`, `x`, `y` its centre relative
to the node) and `a` is the amount. A pan is a scroll: `BK_EV_WHEEL`, whose text is now
`"mods button phase x y"` (the phase is `BK_SCROLL_*`, `x`, `y` where the scroll is, relative to
the node: a finger's scroll moves no pointer there) and whose `b` is the exact pixels of
`BK_scroll_px`.

### Custom nodes

A custom node chooses what a finger does on it with `BK_custom_set_touch(node, mode)`:

| Mode | A finger on the node |
|---|---|
| `BK_TOUCH_MODE_SCROLL` (default) | As on a phone: a tap is a click, a drag scrolls (the node gets `BK_CUSTOM_WHEEL`), the widget-unit strip at its right edge is its scrollbar's press and drag, two fingers pan, pinch and turn |
| `BK_TOUCH_MODE_POINTER` | The pointer at once: press, drag, release are the node's (Dear ImGui's sliders and window moves; the ImGui adapter sets it). Two fingers still pan, pinch and turn |
| `BK_TOUCH_MODE_RAW` | Every contact that begins over the node arrives as `BK_CUSTOM_TOUCH`, any number at once; the node does its own gestures |

Pinches and turns (two fingers or a trackpad) arrive as `BK_CUSTOM_MAGNIFY` / `BK_CUSTOM_ROTATE`.
`BKCustomEvent` has the fields for them: `touch_id` and `touch_phase` (`BK_TOUCH_*`) for
`BK_CUSTOM_TOUCH`, `magnify` and `rotate` for the gestures, with `x`, `y` relative to the node.

```c
static int paint_event(void *data, BKNode *node, const BKCustomEvent *e)
{
  Paint *p = data;
  switch (e->type) {
    case BK_CUSTOM_TOUCH: /* one finger of many, as it is */
      if (e->touch_phase == BK_TOUCH_DOWN) stroke_begin(p, e->touch_id, e->x, e->y);
      else if (e->touch_phase == BK_TOUCH_MOVE) stroke_add(p, e->touch_id, e->x, e->y);
      else stroke_end(p, e->touch_id, e->touch_phase == BK_TOUCH_CANCEL);
      break;
    case BK_CUSTOM_MAGNIFY: /* a trackpad pinch (raw fingers make none) */
      p->zoom *= 1.0f + e->magnify;
      break;
    default:
      return 0;
  }
  BK_node_invalidate(node);
  return 1;
}

BKCustomClass cls = {.event = paint_event, .draw = paint_draw};
BK_custom_set(node, &cls, paint);
BK_custom_set_touch(node, BK_TOUCH_MODE_RAW);
```

## Platforms

| Platform | Touch screen | Trackpad pinch and turn |
|---|---|---|
| Windows (`kit_window_win32.c`) | `WM_POINTER` (Windows 8 and later). Touch only: pen and mouse stay mouse messages. Windows makes no promoted mouse clicks or press-and-hold right click of the handled contacts. Physical-pixel positions are mapped to the client as the mouse's; `WM_POINTERCAPTURECHANGED` and a system-cancelled contact are `BK_TOUCH_CANCEL`; event times come from the performance counter | Not sent by the host |
| Linux X11 (`kit_window_x11.c`) | XInput 2.2 touch through libXi, loaded at run time (`libXi.so.6`) when the headers were there at build time; touch screens only (direct-touch devices). Without libXi the server emulates the pointer for the first finger, as before | XInput 2.4 pinch (libXi 1.8): `BK_magnify` and `BK_rotate` |
| Linux Wayland (`kit_window_wayland.c`) | `wl_touch` | `zwp_pointer_gestures_v1` pinch (when wayland-protocols has pointer-gestures at build time): `BK_magnify` and `BK_rotate` |
| macOS (`kit_window_cocoa.m`) | - | `magnifyWithEvent:` and `rotateWithEvent:` (`NSEvent.magnification`, `NSEvent.rotation`) |
| Android (`kit_window_android.c`) | Every pointer of a motion event, batched history first | - |
| Your own host, headless | Call `BK_touch`, `BK_magnify`, `BK_rotate` | |

On Android the old Android-only touch code is gone, and so is `BK_window_wm(win, "touch_drag", 1, 0)`:
call `BK_custom_set_touch(node, BK_TOUCH_MODE_POINTER)` on the custom node instead (the Dear ImGui
adapter does this for its node).

## Rust

`blenderui` gives elements three listeners, from the element under the gesture's centre up:

```rust
div()
    .on_pan(|e: &PanEvent, _, cx| {
        // e.delta: logical pixels the content follows the fingers (x right, y down)
        // e.phase: ScrollPhase::Drag, then Fling, then End
        cx.stop_propagation(); // take the drag: nothing scrolls
    })
    .on_pinch(|e: &PinchEvent, _, _| { /* zoom *= 1.0 + e.delta, about e.position */ })
    .on_rotate(|e: &RotateEvent, _, _| { /* e.degrees, positive counter-clockwise */ })
```

- `PanEvent { position, delta: Point<Pixels>, phase: ScrollPhase, modifiers }` comes before the
  scroll; each axis may come as an event of its own, and the `End` one is zero.
- `PinchEvent { position, delta, modifiers, touch }` and `RotateEvent { position, degrees,
  modifiers, touch }`: `touch` is `false` for a trackpad.
- In `rsx!`: `onPan`, `onPinch`, `onRotate`.
- `ScrollWheelEvent::position` is where the scroll is (a finger's scroll moves no pointer), and a
  diagonal finger scroll keeps both axes.

`blenderui-sys` has `BK_EV_GESTURE`, `BK_GESTURE_*`, `BK_FROM_TOUCH`, `BK_TOUCH_*` and the three
functions.

## Other bindings

The other language bindings expose the same calls and the same raw `BK_EV_GESTURE` event (its
text is "mods kind x y"), with the constants for the phases, kinds, modes and `BK_FROM_TOUCH`:

| Binding | Calls |
| --- | --- |
| Python | `ctx.touch(id, phase, x, y, time_ms=0, mods=0)`, `ctx.magnify(x, y, delta, mods=0)`, `ctx.rotate(x, y, degrees, mods=0)`; `EV_GESTURE`, `TOUCH_*`, `GESTURE_*`, `FROM_TOUCH` |
| Lua | `ctx:touch(id, phase, x, y [, time_ms [, mods]])`, `ctx:magnify(x, y, delta [, mods])`, `ctx:rotate(...)`, `node:custom_set_touch(mode)`; raw handlers get the event text as a 4th argument |
| D | `Context.touch(id, phase, x, y, timeMs = 0, mods = 0)`, `Context.magnify`, `Context.rotate`, `setTouch(node, mode)`; `CustomEvent.touchId / touchPhase / magnify / rotate` |
| Ruby | `Context#touch(id, phase, x, y, time_ms = 0, mods = 0)`, `#magnify`, `#rotate`; `node.on(:gesture)` |
| Go | `(*Context).Touch(id, phase, x, y, timeMs, mods)`, `Magnify`, `Rotate`; `EventGesture` |
| Dear ImGui | the adapter sets `BK_TOUCH_MODE_POINTER` on its node and reports fingers as `ImGuiMouseSource_TouchScreen` |

## Testing

Headless, drive contacts as a host would:

- C: call `BK_touch` with ids and phases and `BK_render` between them (`tests/touch_test.c`, run by
  `ctest` as `bui_touch_test`: taps, scrolls and flings, a second finger's gesture, raw contacts,
  cancels, the node editor, menus).
- Rust: `cx.simulate_touch(win.id, id, TouchPhase::Down | Move | Up | Cancel, x, y)` and the
  trackpad's `cx.simulate_magnify(win.id, x, y, delta)` and `cx.simulate_rotate(win.id, x, y,
  degrees)` (`bindings/rust/blenderui/tests/suite/touch.rs`).

```rust
// two fingers spread from 100 px apart to 200 about (150, 100): the element's pinches
// multiply to 2.0
cx.simulate_touch(win.id, 1, TouchPhase::Down, 100.0, 100.0);
cx.simulate_touch(win.id, 2, TouchPhase::Down, 200.0, 100.0);
for i in 1..=5 {
    let d = 10.0 * i as f32;
    cx.simulate_touch(win.id, 1, TouchPhase::Move, 100.0 - d, 100.0);
    cx.simulate_touch(win.id, 2, TouchPhase::Move, 200.0 + d, 100.0);
}
cx.simulate_touch(win.id, 1, TouchPhase::Up, 50.0, 100.0);
cx.simulate_touch(win.id, 2, TouchPhase::Up, 250.0, 100.0);
```
