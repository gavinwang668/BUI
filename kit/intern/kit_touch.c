/* Touch screens (BK_touch): a host's contacts turned into what the widgets know, one model on
 * every platform, Android's (View.onTouchEvent, ScrollView, ViewConfiguration):
 *
 *  - a finger that lands on a widget with a drag of its own (CLS_DRAGS: sliders, fields, editors;
 *    a BK_TOUCH_MODE_POINTER custom node; the scrollbar strip at a page's right edge) is the
 *    pointer at once: motion, press, drags, release, with BK_FROM_TOUCH in the mods;
 *  - anywhere else it waits (TAP): lifted in place it is a click, held still for
 *    DEFAULT_LONG_PRESS_TIMEOUT it is a right press (BK_context_menu), and moved past the touch
 *    slop it scrolls the content under where it went down (BK_scroll_px, BK_SCROLL_DRAG), from
 *    just past the slop so the content does not jump by it (RecyclerView's way). When it lifts the
 *    content flings on at its velocity (kit_fling.c: VelocityTracker, OverScroller), stepped by
 *    BK_render while BK_animating says so. A finger that lands on a fling stops it, and is no tap;
 *  - a second finger cancels what the first one did, as a lost pointer does (bk_cancel_pointer,
 *    Blender's EVT_BUT_CANCEL: a dragged value goes back, nothing is clicked), and the two are a
 *    gesture until one lifts. Their centre scrolls like one finger (GHOST's trackpad Scroll,
 *    Blender's MOUSEPAN), their spread magnifies and their angle rotates about the centre
 *    (GHOST_kTrackpadEventMagnify / Rotate: MOUSEZOOM, MOUSEROTATE). Hosts report the fingers one
 *    at a time, so an update waits until both moved (or one moved twice): a pure pan is no wobble
 *    of the spread and angle between them. Afterwards no finger is the pointer until all lifted;
 *  - a BK_TOUCH_MODE_RAW custom node gets the contacts that begin over it as they are;
 *  - the pointer leaves when a finger that was it lifts: no hover stays behind a finger.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#include <math.h>
#include <string.h>

#include "kit_intern.h"

#define TOUCH_SLOP_DP 8.0f  /* ViewConfiguration.TOUCH_SLOP */
#define LONG_PRESS_S 0.5    /* ViewConfiguration.DEFAULT_LONG_PRESS_TIMEOUT */
#define OFF (-100000)       /* the pointer left the window (where the hosts put a leave) */
#define MIN_MAGNIFY 1e-4f   /* smaller spread changes are no pinch (a pan's rounding) */
#define MIN_DEGREES 0.01f

#ifndef M_PI
#  define M_PI 3.14159265358979323846
#endif

void bk_touch_init(BKTouch *t)
{
  memset(t, 0, sizeof(*t));
  t->lead = t->pair[0] = t->pair[1] = -1;
}

void bk_touch_forget(BKTouch *t, const BKNode *subtree)
{
  for (int i = 0; i < BK_MAX_TOUCH; i++) {
    BKContact *c = &t->c[i];
    if (c->role == BK_CONTACT_RAW && bk_is_within(c->raw, subtree)) {
      c->role = BK_CONTACT_IDLE; /* its node is gone: the rest of the contact is nobody's */
      c->raw = NULL;
    }
  }
  if (t->pinch_target && bk_is_within(t->pinch_target, subtree)) {
    t->pinch_target = NULL; /* the gesture goes on with what is under its centre */
  }
}

static int px(float v)
{
  return (int)floorf(v + 0.5f);
}

static int find(const BKTouch *t, int id)
{
  for (int i = 0; i < BK_MAX_TOUCH; i++) {
    if (t->c[i].role != BK_CONTACT_FREE && t->c[i].id == id) {
      return i;
    }
  }
  return -1;
}

/* No finger is the pointer any more: it leaves, as the hosts send a mouse leave. */
static void leave(BKContext *ctx)
{
  BK_motion(ctx, OFF, OFF, BK_FROM_TOUCH);
}

/* The content under the scroll's anchor follows the fingers by (fx, fy) pixels. */
static int scroll(BKContext *ctx, float fx, float fy, int phase, int mods)
{
  BKTouch *t = &ctx->touch;
  BK_scroll_phase(ctx, phase);
  const int r = BK_scroll_px(ctx, t->anchor_x, t->anchor_y, -fx, fy, mods | BK_FROM_TOUCH);
  BK_scroll_phase(ctx, BK_SCROLL_WHEEL);
  return r;
}

static int raw(BKContext *ctx, const BKContact *c, int phase, int mods)
{
  if (!c->raw) {
    return 0;
  }
  INPUT_ENTRY(bk_custom_touch(c->raw, c->id, phase, px(c->x), px(c->y), mods | BK_FROM_TOUCH));
}

/* Whether a finger that lands on `n` at x is the pointer at once: the widget drags. */
static bool presses(const BKContext *ctx, const BKNode *n, int x)
{
  if (ctx->modal) {
    return true; /* a tool runs (the eyedropper): the finger is its pointer */
  }
  if (!n || !NODE_ENABLED(n)) {
    return false;
  }
  if (BK_IS_CUSTOM(n) || n->cls == &bk_cls_canvas) {
    /* a page or an embedded UI scrolls, unless it asked; the strip at its right edge is its
     * scrollbar (a list's thumb), so a huge list can be dragged to any place */
    return bk_custom_touch_mode(n) == BK_TOUCH_MODE_POINTER || x >= n->x + n->w - bk_unit();
  }
  return (n->cls->flags & CLS_DRAGS) != 0;
}

/* A first finger (no other on the one-finger path or in a gesture). */
static int begin_lead(BKContext *ctx, int i, double t_ms, int mods)
{
  BKTouch *t = &ctx->touch;
  BKContact *c = &t->c[i];
  const int x = px(c->x), y = px(c->y);
  c->role = BK_CONTACT_LEAD;
  t->lead = i;
  t->down_x = t->last_x = c->x;
  t->down_y = t->last_y = c->y;
  t->anchor_x = x;
  t->anchor_y = y;
  bk_vt_clear(&t->vt);
  bk_vt_add(&t->vt, t_ms, c->x, c->y);
  if (t->fling_t0 > 0.0) { /* caught: the finger holds the content, it clicks nothing */
    t->fling_t0 = 0.0;
    t->state = BK_TOUCH_SCROLL;
    return 1;
  }
  if (presses(ctx, BK_hit_at(ctx, x, y), x)) {
    t->state = BK_TOUCH_PRESS;
    BK_motion(ctx, x, y, mods | BK_FROM_TOUCH);
    BK_press(ctx, x, y, mods | BK_FROM_TOUCH | BK_BUTTON1);
    return 1;
  }
  t->state = BK_TOUCH_TAP; /* the kit hears of it once it is known which */
  t->long_press_at = bk_time() + LONG_PRESS_S;
  bk_ctx_arm_timer(ctx);
  return 0;
}

static void pair_metrics(const BKTouch *t, float *r_cx, float *r_cy, float *r_dist, float *r_angle)
{
  const BKContact *a = &t->c[t->pair[0]], *b = &t->c[t->pair[1]];
  *r_cx = 0.5f * (a->x + b->x);
  *r_cy = 0.5f * (a->y + b->y);
  *r_dist = hypotf(b->x - a->x, b->y - a->y);
  *r_angle = atan2f(b->y - a->y, b->x - a->x);
}

/* A second finger: the first one's interaction ends with nothing applied, the two are a gesture. */
static int begin_pinch(BKContext *ctx, int i, int mods)
{
  BKTouch *t = &ctx->touch;
  const int first = t->lead;
  const BKTouchState was = t->state;
  int r = 0;
  if (was == BK_TOUCH_PRESS) {
    r = bk_cancel_pointer(ctx, mods | BK_FROM_TOUCH);
    leave(ctx);
  }
  t->c[first].role = t->c[i].role = BK_CONTACT_PAIR;
  t->pair[0] = first;
  t->pair[1] = i;
  t->lead = -1;
  t->state = BK_TOUCH_PINCH;
  t->moved = 0;
  t->long_press_at = 0.0;
  float cx, cy;
  pair_metrics(t, &cx, &cy, &t->dist, &t->angle);
  /* from the centre now, also mid scroll: the point it followed is not where the centre is */
  t->last_x = cx;
  t->last_y = cy;
  if (was != BK_TOUCH_SCROLL) {
    t->anchor_x = px(cx);
    t->anchor_y = px(cy);
  }
  t->pinch_target = bk_gesture_target(ctx, px(cx), px(cy));
  return r | 1;
}

static int pinch_update(BKContext *ctx, int mods)
{
  BKTouch *t = &ctx->touch;
  float cx, cy, dist, angle;
  pair_metrics(t, &cx, &cy, &dist, &angle);
  const float fx = cx - t->last_x, fy = cy - t->last_y;
  const float magnify = t->dist > 1.0f ? dist / t->dist - 1.0f : 0.0f;
  float turn = angle - t->angle;
  while (turn > (float)M_PI) {
    turn -= 2.0f * (float)M_PI;
  }
  while (turn < -(float)M_PI) {
    turn += 2.0f * (float)M_PI;
  }
  const float degrees = -turn * (180.0f / (float)M_PI); /* y points down: a counter-clockwise turn lowers the angle */
  t->last_x = cx;
  t->last_y = cy;
  t->dist = dist;
  t->angle = angle;
  t->moved = 0;
  int r = 0;
  if (fx != 0.0f || fy != 0.0f) {
    r |= scroll(ctx, fx, fy, BK_SCROLL_DRAG, mods);
  }
  const int x = px(cx), y = px(cy);
  BKNode *target = t->pinch_target ? t->pinch_target : bk_gesture_target(ctx, x, y);
  if (fabsf(magnify) > MIN_MAGNIFY) {
    r |= bk_gesture(ctx, target, x, y, BK_GESTURE_MAGNIFY, magnify, mods | BK_FROM_TOUCH);
  }
  if (fabsf(degrees) > MIN_DEGREES) {
    r |= bk_gesture(ctx, target, x, y, BK_GESTURE_ROTATE, degrees, mods | BK_FROM_TOUCH);
  }
  return r;
}

static void fling_start(BKContext *ctx, double t_ms)
{
  BKTouch *t = &ctx->touch;
  float vx, vy;
  if (!bk_vt_velocity(&t->vt, t_ms, &vx, &vy)) {
    return; /* it rested before lifting */
  }
  const bool fx = bk_fling_start(&t->fling_x, vx, bk_scale_fac);
  const bool fy = bk_fling_start(&t->fling_y, vy, bk_scale_fac);
  if (!fx && !fy) {
    return;
  }
  const BKFling none = {0.0f, 1.0f};
  if (!fx) {
    t->fling_x = none;
  }
  if (!fy) {
    t->fling_y = none;
  }
  t->fling_done_x = t->fling_done_y = 0.0f;
  t->fling_t0 = bk_time();
  bk_ctx_request(ctx); /* the frames that step it (BK_animating) */
}

static int lead_move(BKContext *ctx, BKContact *c, double t_ms, int mods)
{
  BKTouch *t = &ctx->touch;
  const int m = mods | BK_FROM_TOUCH;
  switch (t->state) {
    case BK_TOUCH_PRESS:
      return BK_motion(ctx, px(c->x), px(c->y), m | BK_BUTTON1);
    case BK_TOUCH_TAP: {
      const float dx = c->x - t->down_x, dy = c->y - t->down_y, slop = TOUCH_SLOP_DP * bk_scale_fac;
      bk_vt_add(&t->vt, t_ms, c->x, c->y);
      if (dx * dx + dy * dy <= slop * slop) {
        return 0;
      }
      /* not a click: a scroll, from just past the slop (RecyclerView's dy -= mTouchSlop: the slop
       * is the finger's, not the content's, so the content does not jump by it) */
      t->state = BK_TOUCH_SCROLL;
      t->long_press_at = 0.0;
      t->last_x = t->down_x + (fabsf(dx) > slop ? copysignf(slop, dx) : 0.0f);
      t->last_y = t->down_y + (fabsf(dy) > slop ? copysignf(slop, dy) : 0.0f);
      break;
    }
    case BK_TOUCH_SCROLL:
      bk_vt_add(&t->vt, t_ms, c->x, c->y);
      break;
    default:
      return 0;
  }
  const float fx = c->x - t->last_x, fy = c->y - t->last_y;
  t->last_x = c->x;
  t->last_y = c->y;
  return fx != 0.0f || fy != 0.0f ? scroll(ctx, fx, fy, BK_SCROLL_DRAG, mods) : 0;
}

/* The lead lifted (UP) or was taken back (CANCEL). */
static int lead_end(BKContext *ctx, BKContact *c, int phase, double t_ms, int mods)
{
  BKTouch *t = &ctx->touch;
  const int m = mods | BK_FROM_TOUCH, x = px(c->x), y = px(c->y);
  const bool up = phase == BK_TOUCH_UP;
  int r = 0;
  switch (t->state) {
    case BK_TOUCH_TAP:
      if (up) { /* lifted in place: the click */
        BK_motion(ctx, x, y, m);
        BK_press(ctx, x, y, m | BK_BUTTON1);
        BK_release(ctx, x, y, m);
        leave(ctx);
        r = 1;
      }
      break;
    case BK_TOUCH_PRESS:
      r = up ? BK_release(ctx, x, y, m) : bk_cancel_pointer(ctx, m);
      leave(ctx);
      break;
    case BK_TOUCH_SCROLL:
      r = scroll(ctx, 0.0f, 0.0f, BK_SCROLL_END, mods); /* a list lets go of a stretch */
      if (up) {
        bk_vt_add(&t->vt, t_ms, c->x, c->y);
        fling_start(ctx, t_ms);
      }
      break;
    default: /* SPENT: a long press opened its menu */
      break;
  }
  t->lead = -1;
  t->long_press_at = 0.0;
  return r;
}

/* One of the pair lifted or was taken back: the gesture ends, the other finger does nothing more. */
static int pair_end(BKContext *ctx, int i, int phase, int mods)
{
  BKTouch *t = &ctx->touch;
  const unsigned bit = i == t->pair[0] ? 1u : 2u;
  int r = 0;
  if (phase == BK_TOUCH_UP && (t->moved & bit)) {
    r |= pinch_update(ctx, mods); /* the move it held; a cancel drops it with the rest */
  }
  r |= scroll(ctx, 0.0f, 0.0f, BK_SCROLL_END, mods);
  const int other = t->pair[0] == i ? t->pair[1] : t->pair[0];
  t->c[other].role = BK_CONTACT_IDLE;
  t->pair[0] = t->pair[1] = -1;
  t->pinch_target = NULL;
  t->state = BK_TOUCH_SPENT;
  return r;
}

static int contact_down(BKContext *ctx, int id, float x, float y, double t_ms, int mods)
{
  BKTouch *t = &ctx->touch;
  int i = 0;
  while (i < BK_MAX_TOUCH && t->c[i].role != BK_CONTACT_FREE) {
    i++;
  }
  if (i == BK_MAX_TOUCH) {
    return 0; /* an eleventh finger: not followed */
  }
  BKContact *c = &t->c[i];
  c->id = id;
  c->x = x;
  c->y = y;
  c->raw = NULL;
  c->role = BK_CONTACT_IDLE;
  if (!ctx->modal && !ctx->npopups) { /* menus and tools take every finger as the pointer's */
    BKNode *hit = BK_hit_at(ctx, px(x), px(y));
    if (BK_IS_CUSTOM(hit) && NODE_ENABLED(hit) && bk_custom_touch_mode(hit) == BK_TOUCH_MODE_RAW) {
      c->role = BK_CONTACT_RAW;
      c->raw = hit;
      return raw(ctx, c, BK_TOUCH_DOWN, mods);
    }
  }
  switch (t->state) {
    case BK_TOUCH_IDLE:
      return begin_lead(ctx, i, t_ms, mods);
    case BK_TOUCH_TAP:
    case BK_TOUCH_PRESS:
    case BK_TOUCH_SCROLL:
      return begin_pinch(ctx, i, mods);
    default:
      return 0; /* a third finger, or one after a gesture: followed, unused */
  }
}

/* MOVE, UP or CANCEL of contact i, at its new position. */
static int contact_update(BKContext *ctx, int i, int phase, double t_ms, int mods)
{
  BKTouch *t = &ctx->touch;
  BKContact *c = &t->c[i];
  switch (c->role) {
    case BK_CONTACT_RAW:
      return raw(ctx, c, phase, mods);
    case BK_CONTACT_LEAD:
      return phase == BK_TOUCH_MOVE ? lead_move(ctx, c, t_ms, mods) :
                                      lead_end(ctx, c, phase, t_ms, mods);
    case BK_CONTACT_PAIR: {
      if (phase != BK_TOUCH_MOVE) {
        return pair_end(ctx, i, phase, mods);
      }
      const unsigned bit = i == t->pair[0] ? 1u : 2u;
      const bool again = (t->moved & bit) != 0;
      t->moved |= bit;
      return again || t->moved == 3u ? pinch_update(ctx, mods) : 0;
    }
    default:
      return 0;
  }
}

/* Contact i is gone: its slot frees, and with the last finger that was not raw the fingers are
 * free to be the pointer again. */
static void contact_free(BKTouch *t, int i)
{
  t->c[i].role = BK_CONTACT_FREE;
  t->c[i].raw = NULL;
  for (int k = 0; k < BK_MAX_TOUCH; k++) {
    const BKContactRole role = t->c[k].role;
    if (role != BK_CONTACT_FREE && role != BK_CONTACT_RAW) {
      return;
    }
  }
  t->state = BK_TOUCH_IDLE;
  t->lead = -1;
}

int BK_touch(BKContext *ctx, int id, int phase, float x, float y, double time_ms, int mods)
{
  if (!ctx || phase < BK_TOUCH_DOWN || phase > BK_TOUCH_CANCEL) {
    return 0;
  }
  BKTouch *t = &ctx->touch;
  const double t_ms = time_ms > 0.0 ? time_ms : bk_time() * 1000.0;
  mods &= BK_SHIFT | BK_CTRL | BK_ALT; /* buttons and BK_FROM_TOUCH are the kit's to set */
  int i = find(t, id);
  if (phase == BK_TOUCH_DOWN) {
    int r = 0;
    if (i >= 0) { /* its lift was lost: what it did is not kept */
      r = contact_update(ctx, i, BK_TOUCH_CANCEL, t_ms, mods);
      contact_free(t, i);
    }
    return contact_down(ctx, id, x, y, t_ms, mods) | r;
  }
  if (i < 0) {
    return 0; /* not followed (an eleventh finger, or one that began before a cancel) */
  }
  BKContact *c = &t->c[i];
  if (phase != BK_TOUCH_CANCEL) { /* a cancel has no place of its own (wl_touch.cancel) */
    c->x = x;
    c->y = y;
  }
  const int r = contact_update(ctx, i, phase, t_ms, mods);
  if (phase != BK_TOUCH_MOVE) {
    contact_free(t, i);
  }
  return r;
}

void bk_touch_cancel_all(BKContext *ctx)
{
  BKTouch *t = &ctx->touch;
  bool down = false;
  for (int i = 0; i < BK_MAX_TOUCH; i++) {
    BKContact *c = &t->c[i];
    if (c->role == BK_CONTACT_RAW) {
      raw(ctx, c, BK_TOUCH_CANCEL, 0);
      c->role = BK_CONTACT_IDLE;
      c->raw = NULL;
    }
    else if (c->role == BK_CONTACT_LEAD) {
      lead_end(ctx, c, BK_TOUCH_CANCEL, 0.0, 0);
      c->role = BK_CONTACT_IDLE;
    }
    else if (c->role == BK_CONTACT_PAIR && t->state == BK_TOUCH_PINCH) {
      pair_end(ctx, i, BK_TOUCH_CANCEL, 0);
      c->role = BK_CONTACT_IDLE;
    }
    down |= c->role != BK_CONTACT_FREE;
  }
  /* the fingers still down are followed until they lift (their ups may never come: a new down
   * with the same id replaces one) */
  t->state = down ? BK_TOUCH_SPENT : BK_TOUCH_IDLE;
}

double bk_touch_due(const BKContext *ctx)
{
  return ctx->touch.state == BK_TOUCH_TAP ? ctx->touch.long_press_at : 0.0;
}

void bk_touch_tick(BKContext *ctx)
{
  BKTouch *t = &ctx->touch;
  if (t->state != BK_TOUCH_TAP || t->long_press_at <= 0.0 || bk_time() + 0.002 < t->long_press_at) {
    return;
  }
  /* held in place: a right press (the context menu), and the finger does nothing more */
  t->long_press_at = 0.0;
  const int x = px(t->down_x), y = px(t->down_y);
  if (BK_context_menu(ctx, x, y, BK_FROM_TOUCH)) {
    BK_button(ctx, x, y, 3, 1, BK_FROM_TOUCH);
    t->state = BK_TOUCH_SPENT;
  }
}

void bk_touch_frame(BKContext *ctx)
{
  BKTouch *t = &ctx->touch;
  bk_touch_tick(ctx); /* hosts without timers: the long press comes with a render */
  if (t->fling_t0 <= 0.0) {
    return;
  }
  const double elapsed = (bk_time() - t->fling_t0) * 1000.0;
  bool done_x, done_y;
  const float x = bk_fling_at(&t->fling_x, elapsed, &done_x);
  const float y = bk_fling_at(&t->fling_y, elapsed, &done_y);
  scroll(ctx, x - t->fling_done_x, y - t->fling_done_y, BK_SCROLL_FLING, 0);
  t->fling_done_x = x;
  t->fling_done_y = y;
  if (done_x && done_y) {
    t->fling_t0 = 0.0;
  }
}

bool bk_touch_animating(const BKContext *ctx)
{
  return ctx->touch.fling_t0 > 0.0;
}
