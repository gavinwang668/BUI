/* Touch screens and trackpad gestures (BK_touch, BK_magnify, BK_rotate), driven the way a window
 * host feeds them: taps, the pointer on widgets that drag, one-finger scrolls and their fling, a
 * long press, a second finger's cancel and gesture, raw contacts of custom nodes, lost contacts.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#ifdef _WIN32
#  include <windows.h>
#else
#  include <unistd.h>
#endif

#include "BUI_kit.h"
#include "GPU_raster.h"

static int g_fail = 0;
#define CHECK(c) \
  do { \
    if (!(c)) { \
      printf("FAIL %s:%d: %s\n", __FILE__, __LINE__, #c); \
      g_fail = 1; \
    } \
  } while (0)

enum { W = 400, H = 300 };
#define TIMER_FRAME (-1) /* the context's frame timer (custom nodes, the long press) */

static void sleep_ms(int ms)
{
#ifdef _WIN32
  Sleep((DWORD)ms);
#else
  usleep((useconds_t)ms * 1000);
#endif
}

/* -- a host that logs what the kit tells it ----------------------------------------------- */

typedef struct Ev {
  BKNode *node;
  int kind;
  char text[64];
  double a, b;
} Ev;

static Ev g_ev[512];
static int g_nev, g_warps;

static void host_emit(void *data, BKNode *node, int kind, void *ref, double a, double b)
{
  (void)data;
  if (g_nev == (int)(sizeof(g_ev) / sizeof(g_ev[0]))) {
    return;
  }
  Ev *e = &g_ev[g_nev++];
  e->node = node;
  e->kind = kind;
  e->a = a;
  e->b = b;
  e->text[0] = 0;
  if (ref && kind != BK_EV_CALL && kind != BK_EV_A11Y_ACTION && kind != BK_EV_CHANGE &&
      kind != BK_EV_CLICK)
  {
    snprintf(e->text, sizeof(e->text), "%s", (const char *)ref);
  }
}

static void host_warp(void *data, int x, int y)
{
  (void)data;
  (void)x;
  (void)y;
  g_warps++;
}

static void host_noop(void *data)
{
  (void)data;
}

static int count(int kind)
{
  int n = 0;
  for (int i = 0; i < g_nev; i++) {
    n += g_ev[i].kind == kind;
  }
  return n;
}

static const Ev *last(int kind)
{
  for (int i = g_nev - 1; i >= 0; i--) {
    if (g_ev[i].kind == kind) {
      return &g_ev[i];
    }
  }
  return NULL;
}

static int text_int(const Ev *e, int field)
{
  int v[6] = {0};
  sscanf(e->text, "%d %d %d %d %d %d", &v[0], &v[1], &v[2], &v[3], &v[4], &v[5]);
  return v[field];
}

static BKContext *context(BKNode *root)
{
  BKContext *ctx = BK_context_new(root);
  BKHost host;
  memset(&host, 0, sizeof(host));
  host.request = host_noop;
  host.warp = host_warp;
  host.emit = host_emit;
  BK_context_set_host(ctx, &host);
  g_nev = g_warps = 0;
  return ctx;
}

static BuiRaster *g_raster;

static void render(BKContext *ctx)
{
  BK_render(ctx, g_raster, W, H, 0);
}

static void centre(BKNode *n, int *x, int *y)
{
  int r[4];
  BK_node_rect(n, r);
  *x = r[0] + r[2] / 2;
  *y = r[1] + r[3] / 2;
}

/* a finger's whole path from (x0, y0) to (x1, y1), `steps` moves `dt` ms apart from `t` */
static double drag(BKContext *ctx, int id, float x0, float y0, float x1, float y1, int steps, double t, double dt)
{
  BK_touch(ctx, id, BK_TOUCH_DOWN, x0, y0, t, 0);
  for (int i = 1; i <= steps; i++) {
    t += dt;
    BK_touch(ctx, id, BK_TOUCH_MOVE, x0 + (x1 - x0) * i / steps, y0 + (y1 - y0) * i / steps, t, 0);
  }
  return t;
}

/* -- widgets -------------------------------------------------------------------------------- */

static void test_tap_and_pointer(void)
{
  BKNode *col = BK_node_new("column");
  BKNode *button = BK_node_new("button");
  BK_set_str(button, "text", "Go");
  BKNode *num = BK_node_new("number");
  BK_set_num(num, "max", 1000);
  BK_set_num(num, "value", 10);
  BK_node_add(col, button, -1);
  BK_node_add(col, num, -1);
  BKContext *ctx = context(col);
  render(ctx);
  int bx, by, nx, ny;
  centre(button, &bx, &by);
  centre(num, &nx, &ny);

  /* a tap on a button: one click, nothing hovered once the finger is gone */
  BK_touch(ctx, 1, BK_TOUCH_DOWN, (float)bx, (float)by, 0, 0);
  CHECK(count(BK_EV_CLICK) == 0); /* not before it lifts: it could still become a scroll */
  BK_touch(ctx, 1, BK_TOUCH_UP, (float)bx + 2, (float)by, 0, 0);
  CHECK(count(BK_EV_CLICK) == 1);
  CHECK(BK_hover(ctx) == NULL);

  /* a finger that moves past the slop is a scroll, not a click */
  g_nev = 0;
  const double t = drag(ctx, 2, (float)bx, (float)by, (float)bx, (float)by + 40, 4, 1000.0, 10.0);
  BK_touch(ctx, 2, BK_TOUCH_UP, (float)bx, (float)by + 40, t + 100.0, 0); /* rested: no fling */
  CHECK(count(BK_EV_CLICK) == 0);
  CHECK(!BK_animating(ctx));

  /* a number field drags at once (the pointer), with no warp at the window edge */
  g_nev = 0;
  drag(ctx, 3, (float)nx, (float)ny, (float)W - 2, (float)ny, 8, 0, 0);
  const double dragged = BK_get_num(num, "value");
  CHECK(dragged > 10.0);
  BK_touch(ctx, 3, BK_TOUCH_UP, (float)W - 2, (float)ny, 0, 0);
  CHECK(g_warps == 0);
  CHECK(BK_get_num(num, "value") == dragged);
  CHECK(BK_hover(ctx) == NULL);
  CHECK(BK_focus(ctx) == NULL); /* a drag is no click: no text editing */

  /* a second finger cancels the drag: the value goes back, as with Escape */
  BK_set_num(num, "value", 10);
  drag(ctx, 4, (float)nx, (float)ny, (float)nx + 60, (float)ny, 4, 0, 0);
  CHECK(BK_get_num(num, "value") > 10.0);
  BK_touch(ctx, 5, BK_TOUCH_DOWN, (float)nx, (float)ny + 80, 0, 0);
  CHECK(BK_get_num(num, "value") == 10.0);
  BK_touch(ctx, 4, BK_TOUCH_UP, (float)nx + 60, (float)ny, 0, 0);
  /* the finger left behind is no pointer: lifting it over the button clicks nothing */
  g_nev = 0;
  BK_touch(ctx, 5, BK_TOUCH_MOVE, (float)bx, (float)by, 0, 0);
  BK_touch(ctx, 5, BK_TOUCH_UP, (float)bx, (float)by, 0, 0);
  CHECK(count(BK_EV_CLICK) == 0);
  CHECK(BK_focus(ctx) == NULL);

  /* a cancelled contact keeps nothing, and its later events do nothing */
  drag(ctx, 6, (float)nx, (float)ny, (float)nx + 60, (float)ny, 4, 0, 0);
  BK_touch(ctx, 6, BK_TOUCH_CANCEL, (float)nx + 60, (float)ny, 0, 0);
  CHECK(BK_get_num(num, "value") == 10.0);
  CHECK(BK_touch(ctx, 6, BK_TOUCH_MOVE, (float)nx + 90, (float)ny, 0, 0) == 0);

  /* BK_cancel (a lost capture, the window losing focus) cancels the finger's drag too */
  drag(ctx, 7, (float)nx, (float)ny, (float)nx + 60, (float)ny, 4, 0, 0);
  CHECK(BK_cancel(ctx, 0) != 0);
  CHECK(BK_get_num(num, "value") == 10.0);
  CHECK(BK_touch(ctx, 7, BK_TOUCH_MOVE, (float)nx + 90, (float)ny, 0, 0) == 0);
  BK_touch(ctx, 7, BK_TOUCH_UP, (float)nx + 90, (float)ny, 0, 0);

  /* a lost lift: the same id going down again ends what it did first */
  drag(ctx, 8, (float)nx, (float)ny, (float)nx + 60, (float)ny, 4, 0, 0);
  BK_touch(ctx, 8, BK_TOUCH_DOWN, (float)bx, (float)by, 0, 0);
  CHECK(BK_get_num(num, "value") == 10.0);
  BK_touch(ctx, 8, BK_TOUCH_UP, (float)bx, (float)by, 0, 0);

  /* two taps in a row on the number field: a double click (its text editing) */
  BK_touch(ctx, 9, BK_TOUCH_DOWN, (float)nx, (float)ny, 0, 0);
  BK_touch(ctx, 9, BK_TOUCH_UP, (float)nx, (float)ny, 0, 0);
  CHECK(BK_focus(ctx) == num); /* a number field's click edits its text */
  BK_key(ctx, "", "Escape", 0);

  BK_context_free(ctx);
  BK_node_free(col);
}

/* -- scrolls on an "events" page (as the Rust and Python hosts draw theirs) ------------------- */

static void test_scroll_and_fling(void)
{
  BKNode *page = BK_node_new("canvas");
  BK_set_num(page, "events", 1);
  BKNode *root = BK_node_new("frame");
  BK_node_add(root, page, -1);
  BK_geom_set_manager(page, BK_GEOM_PLACE);
  BK_geom_set(page, "relwidth", 1.0);
  BK_geom_set(page, "relheight", 1.0);
  BKContext *ctx = context(root);
  render(ctx);

  /* the scroll starts past the slop and follows the finger exactly, at the point it began */
  double t = drag(ctx, 1, 100, 200, 100, 100, 10, 1000.0, 10.0);
  float total = 0.0f;
  int drags = 0, last_x = -1, last_y = -1;
  for (int i = 0; i < g_nev; i++) {
    if (g_ev[i].kind == BK_EV_WHEEL && text_int(&g_ev[i], 2) == BK_SCROLL_DRAG) {
      total += (float)g_ev[i].b;
      drags++;
      CHECK((text_int(&g_ev[i], 0) & BK_FROM_TOUCH) != 0);
      last_x = text_int(&g_ev[i], 3);
      last_y = text_int(&g_ev[i], 4);
    }
  }
  CHECK(drags > 0);
  /* 100 px less the 8 px slop, the content up with the finger (negative: it goes up) */
  CHECK(fabsf(total + 92.0f) < 1.0f);
  CHECK(last_x == 100 && last_y == 200);
  CHECK(count(BK_EV_PRESS) == 0 && count(BK_EV_MOTION) == 0); /* a scroll is no pointer, no hover */

  /* lifted while moving: the end, then a fling on Android's curve, stepped by renders */
  g_nev = 0;
  BK_touch(ctx, 1, BK_TOUCH_UP, 100, 100, t, 0);
  const Ev *end = last(BK_EV_WHEEL);
  CHECK(end && text_int(end, 2) == BK_SCROLL_END && end->b == 0.0);
  CHECK(BK_animating(ctx));
  float flung = 0.0f;
  for (int i = 0; i < 400 && BK_animating(ctx); i++) {
    sleep_ms(4);
    const int before = g_nev;
    render(ctx);
    for (int k = before; k < g_nev; k++) {
      if (g_ev[k].kind == BK_EV_WHEEL) {
        CHECK(text_int(&g_ev[k], 2) == BK_SCROLL_FLING);
        flung += (float)g_ev[k].b;
      }
    }
  }
  CHECK(!BK_animating(ctx));
  CHECK(flung < -50.0f); /* on up the way the finger went */

  /* a finger that rested before lifting does not fling */
  g_nev = 0;
  t = drag(ctx, 2, 100, 200, 100, 100, 10, 5000.0, 10.0);
  BK_touch(ctx, 2, BK_TOUCH_UP, 100, 100, t + 200.0, 0);
  CHECK(!BK_animating(ctx));

  /* a finger that lands on a fling stops it, and is no tap */
  t = drag(ctx, 3, 100, 200, 100, 100, 10, 9000.0, 10.0);
  BK_touch(ctx, 3, BK_TOUCH_UP, 100, 100, t, 0);
  CHECK(BK_animating(ctx));
  g_nev = 0;
  BK_touch(ctx, 4, BK_TOUCH_DOWN, 150, 150, 0, 0);
  CHECK(!BK_animating(ctx));
  BK_touch(ctx, 4, BK_TOUCH_UP, 150, 150, 0, 0);
  CHECK(count(BK_EV_PRESS) == 0);

  /* a tap: press and release with BK_FROM_TOUCH, then the pointer leaves */
  g_nev = 0;
  BK_touch(ctx, 5, BK_TOUCH_DOWN, 150, 150, 0, 0);
  BK_touch(ctx, 5, BK_TOUCH_UP, 150, 150, 0, 0);
  const Ev *press = last(BK_EV_PRESS);
  CHECK(press && (text_int(press, 0) & BK_FROM_TOUCH) && text_int(press, 1) == 1);
  CHECK(count(BK_EV_RELEASE) == 1);
  CHECK(last(BK_EV_LEAVE) != NULL);
  CHECK(BK_hover(ctx) == NULL);

  /* held still: a right press (the context menu), and lifting clicks nothing */
  g_nev = 0;
  BK_touch(ctx, 6, BK_TOUCH_DOWN, 150, 150, 0, 0);
  BK_timer(ctx, TIMER_FRAME); /* too early: nothing */
  CHECK(count(BK_EV_PRESS) == 0);
  sleep_ms(520);
  BK_timer(ctx, TIMER_FRAME);
  press = last(BK_EV_PRESS);
  CHECK(press && text_int(press, 1) == 3);
  BK_touch(ctx, 6, BK_TOUCH_UP, 150, 150, 0, 0);
  CHECK(count(BK_EV_PRESS) == 1);

  /* -- two fingers: a pure pan scrolls and is no pinch or turn ---------------------------- */
  g_nev = 0;
  BK_touch(ctx, 10, BK_TOUCH_DOWN, 100, 150, 0, 0);
  BK_touch(ctx, 11, BK_TOUCH_DOWN, 200, 150, 0, 0);
  for (int i = 1; i <= 5; i++) { /* the host reports the fingers one by one */
    BK_touch(ctx, 10, BK_TOUCH_MOVE, 100, 150.0f + 10 * i, 0, 0);
    BK_touch(ctx, 11, BK_TOUCH_MOVE, 200, 150.0f + 10 * i, 0, 0);
  }
  total = 0.0f;
  for (int i = 0; i < g_nev; i++) {
    if (g_ev[i].kind == BK_EV_WHEEL && text_int(&g_ev[i], 2) == BK_SCROLL_DRAG) {
      total += (float)g_ev[i].b;
    }
  }
  CHECK(fabsf(total - 50.0f) < 0.5f); /* down with the fingers */
  CHECK(count(BK_EV_GESTURE) == 0);
  CHECK(count(BK_EV_PRESS) == 0);
  BK_touch(ctx, 10, BK_TOUCH_UP, 100, 200, 0, 0);
  BK_touch(ctx, 11, BK_TOUCH_UP, 200, 200, 0, 0);

  /* a spread: magnify about the centre, the steps' product the whole spread */
  g_nev = 0;
  BK_touch(ctx, 12, BK_TOUCH_DOWN, 150, 100, 0, 0);
  BK_touch(ctx, 13, BK_TOUCH_DOWN, 250, 100, 0, 0);
  for (int i = 1; i <= 5; i++) {
    BK_touch(ctx, 12, BK_TOUCH_MOVE, 150.0f - 10 * i, 100, 0, 0);
    BK_touch(ctx, 13, BK_TOUCH_MOVE, 250.0f + 10 * i, 100, 0, 0);
  }
  double scale = 1.0;
  int gestures = 0;
  for (int i = 0; i < g_nev; i++) {
    if (g_ev[i].kind == BK_EV_GESTURE) {
      CHECK(text_int(&g_ev[i], 1) == BK_GESTURE_MAGNIFY);
      CHECK((text_int(&g_ev[i], 0) & BK_FROM_TOUCH) != 0);
      CHECK(text_int(&g_ev[i], 2) == 200 && text_int(&g_ev[i], 3) == 100);
      scale *= 1.0 + g_ev[i].a;
      gestures++;
    }
  }
  CHECK(gestures == 5);
  CHECK(fabs(scale - 2.0) < 1e-3); /* 100 px apart to 200 */
  BK_touch(ctx, 12, BK_TOUCH_UP, 100, 100, 0, 0);
  BK_touch(ctx, 13, BK_TOUCH_UP, 300, 100, 0, 0);

  /* a quarter turn counter-clockwise (on screen) is +90 degrees */
  g_nev = 0;
  BK_touch(ctx, 14, BK_TOUCH_DOWN, 150, 150, 0, 0);
  BK_touch(ctx, 15, BK_TOUCH_DOWN, 250, 150, 0, 0);
  double degrees = 0.0;
  for (int i = 1; i <= 9; i++) {
    const double a = -(M_PI / 2) * i / 9; /* y points down: counter-clockwise is negative */
    BK_touch(ctx, 14, BK_TOUCH_MOVE, (float)(200 - 50 * cos(a)), (float)(150 - 50 * sin(a)), 0, 0);
    BK_touch(ctx, 15, BK_TOUCH_MOVE, (float)(200 + 50 * cos(a)), (float)(150 + 50 * sin(a)), 0, 0);
  }
  for (int i = 0; i < g_nev; i++) {
    if (g_ev[i].kind == BK_EV_GESTURE && text_int(&g_ev[i], 1) == BK_GESTURE_ROTATE) {
      degrees += g_ev[i].a;
    }
  }
  CHECK(fabs(degrees - 90.0) < 0.5);
  /* a third finger is followed and ignored */
  CHECK(BK_touch(ctx, 16, BK_TOUCH_DOWN, 50, 50, 0, 0) == 0);
  BK_touch(ctx, 16, BK_TOUCH_UP, 50, 50, 0, 0);
  BK_touch(ctx, 14, BK_TOUCH_UP, 0, 0, 0, 0);
  g_nev = 0;
  BK_touch(ctx, 15, BK_TOUCH_UP, 250, 150, 0, 0); /* the finger left of a gesture clicks nothing */
  CHECK(count(BK_EV_PRESS) == 0);

  /* trackpads: BK_magnify / BK_rotate, without BK_FROM_TOUCH */
  g_nev = 0;
  BK_magnify(ctx, 120, 130, 0.25f, BK_CTRL | BK_FROM_TOUCH);
  BK_rotate(ctx, 120, 130, -15.0f, 0);
  const Ev *g = &g_ev[0];
  CHECK(g_nev == 2 && g->kind == BK_EV_GESTURE && text_int(g, 0) == BK_CTRL);
  CHECK(text_int(g, 1) == BK_GESTURE_MAGNIFY && fabs(g->a - 0.25) < 1e-6);
  CHECK(text_int(g, 2) == 120 && text_int(g, 3) == 130);
  CHECK(text_int(&g_ev[1], 1) == BK_GESTURE_ROTATE && g_ev[1].a == -15.0);

  BK_context_free(ctx);
  BK_node_free(root);
}

/* -- the node editor: two fingers pan and zoom (MOUSEPAN, MOUSEZOOM), and cancel a press ---- */

static void test_node_editor(void)
{
  BKNode *ed = BK_node_new("nodeeditor");
  for (int i = 0; i < 2; i++) {
    BKNode *n = BK_node_new("graphnode");
    BK_set_str(n, "label", i ? "Math" : "Value");
    BK_set_num(n, "nx", i * 220.0);
    const char *sockets[] = {"o|Value|float", "i|A|float|0.5"};
    BK_set_strs(n, "items", sockets, i ? 2 : 1);
    BK_set_num(n, "selected", 1);
    BK_node_add(ed, n, -1);
  }
  BKContext *ctx = context(ed);
  render(ctx);
  const double zoom = BK_get_num(ed, "zoom"), pany = BK_get_num(ed, "pany");

  /* a finger on empty space presses at once (a box select), and a second finger cancels it:
   * the unmoved press deselects nothing (its release would have) */
  BK_touch(ctx, 1, BK_TOUCH_DOWN, 380, 280, 0, 0);
  BK_touch(ctx, 2, BK_TOUCH_DOWN, 300, 280, 0, 0);
  CHECK(BK_get_num(BK_node_child(ed, 0), "selected") == 1.0);
  CHECK(BK_get_num(BK_node_child(ed, 1), "selected") == 1.0);
  /* moving together pans the view with the fingers */
  for (int i = 1; i <= 4; i++) {
    BK_touch(ctx, 1, BK_TOUCH_MOVE, 380, 280.0f - 5 * i, 0, 0);
    BK_touch(ctx, 2, BK_TOUCH_MOVE, 300, 280.0f - 5 * i, 0, 0);
  }
  CHECK(fabs(BK_get_num(ed, "pany") - (pany - 20.0)) < 0.5);
  CHECK(BK_get_num(ed, "zoom") == zoom);
  /* spreading zooms about their centre */
  BK_touch(ctx, 1, BK_TOUCH_MOVE, 400, 260, 0, 0);
  BK_touch(ctx, 2, BK_TOUCH_MOVE, 280, 260, 0, 0);
  CHECK(fabs(BK_get_num(ed, "zoom") - zoom * 1.5) < 1e-3);
  BK_touch(ctx, 1, BK_TOUCH_UP, 400, 260, 0, 0);
  BK_touch(ctx, 2, BK_TOUCH_UP, 280, 260, 0, 0);
  CHECK(BK_get_num(BK_node_child(ed, 0), "selected") == 1.0);

  /* a trackpad pinch zooms too */
  const double z = BK_get_num(ed, "zoom");
  BK_magnify(ctx, 200, 150, -0.2f, 0);
  CHECK(fabs(BK_get_num(ed, "zoom") - z * 0.8) < 1e-3);

  BK_context_free(ctx);
  BK_node_free(ed);
}

/* -- custom nodes: raw contacts, the default gestures, the pointer mode ---------------------- */

typedef struct Log {
  int type[256], id[256], phase[256], mods[256];
  float x[256], magnify[256], rotate[256], wheel_y[256];
  int n;
} Log;

static int custom_event(void *data, BKNode *node, const BKCustomEvent *e)
{
  (void)node;
  Log *l = (Log *)data;
  if (l->n < 256) {
    l->type[l->n] = e->type;
    l->id[l->n] = e->touch_id;
    l->phase[l->n] = e->touch_phase;
    l->mods[l->n] = e->mods;
    l->x[l->n] = e->x;
    l->magnify[l->n] = e->magnify;
    l->rotate[l->n] = e->rotate;
    l->wheel_y[l->n] = e->wheel_y;
    l->n++;
  }
  return 1;
}

static int find_type(const Log *l, int type)
{
  for (int i = 0; i < l->n; i++) {
    if (l->type[i] == type) {
      return i;
    }
  }
  return -1;
}

static void test_custom(void)
{
  static Log log;
  BKNode *col = BK_node_new("column");
  BKNode *node = BK_node_new("custom");
  BKCustomClass cls;
  memset(&cls, 0, sizeof(cls));
  cls.event = custom_event;
  cls.natural_w = 300;
  cls.natural_h = 200;
  BK_custom_set(node, &cls, &log);
  BK_node_add(col, node, -1);
  BKContext *ctx = context(col);
  render(ctx);
  int r[4];
  BK_node_rect(node, r);
  const float cx = (float)(r[0] + 100), cy = (float)(r[1] + 100);

  /* raw: every contact as it is, two fingers no gesture */
  BK_custom_set_touch(node, BK_TOUCH_MODE_RAW);
  memset(&log, 0, sizeof(log));
  BK_touch(ctx, 7, BK_TOUCH_DOWN, cx, cy, 0, 0);
  BK_touch(ctx, 8, BK_TOUCH_DOWN, cx + 50, cy, 0, 0);
  BK_touch(ctx, 8, BK_TOUCH_MOVE, cx + 80, cy, 0, 0);
  BK_touch(ctx, 7, BK_TOUCH_UP, cx, cy, 0, 0);
  BK_touch(ctx, 8, BK_TOUCH_CANCEL, cx + 80, cy, 0, 0);
  int touches = 0;
  for (int i = 0; i < log.n; i++) {
    if (log.type[i] == BK_CUSTOM_TOUCH) {
      touches++;
      CHECK(log.mods[i] & BK_FROM_TOUCH);
    }
  }
  CHECK(touches == 5);
  CHECK(find_type(&log, BK_CUSTOM_FOCUS) >= 0); /* the first contact takes the focus, as a click */
  int k = find_type(&log, BK_CUSTOM_TOUCH);
  CHECK(k >= 0 && log.id[k] == 7 && log.phase[k] == BK_TOUCH_DOWN && fabsf(log.x[k] - 100.0f) < 1.0f);
  CHECK(log.phase[log.n - 1] == BK_TOUCH_CANCEL && log.id[log.n - 1] == 8);
  CHECK(find_type(&log, BK_CUSTOM_PRESS) < 0 && find_type(&log, BK_CUSTOM_MAGNIFY) < 0);

  /* a raw node freed while a finger is on it: the rest of the contact goes nowhere */
  BK_touch(ctx, 9, BK_TOUCH_DOWN, cx, cy, 0, 0);
  BK_node_remove(col, node);
  memset(&log, 0, sizeof(log));
  BK_touch(ctx, 9, BK_TOUCH_MOVE, cx + 10, cy, 0, 0);
  BK_touch(ctx, 9, BK_TOUCH_UP, cx + 10, cy, 0, 0);
  CHECK(log.n == 0);
  BK_node_add(col, node, -1);
  render(ctx);

  /* the default: a drag scrolls it (wheel notches), two fingers pinch it */
  BK_custom_set_touch(node, BK_TOUCH_MODE_SCROLL);
  memset(&log, 0, sizeof(log));
  drag(ctx, 1, cx, cy, cx, cy + 60, 6, 0, 0);
  BK_touch(ctx, 2, BK_TOUCH_DOWN, cx + 60, cy + 60, 0, 0);
  BK_touch(ctx, 1, BK_TOUCH_MOVE, cx - 30, cy + 60, 0, 0);
  BK_touch(ctx, 2, BK_TOUCH_MOVE, cx + 90, cy + 60, 0, 0);
  BK_touch(ctx, 1, BK_TOUCH_UP, cx - 30, cy + 60, 0, 0);
  BK_touch(ctx, 2, BK_TOUCH_UP, cx + 90, cy + 60, 0, 0);
  k = find_type(&log, BK_CUSTOM_WHEEL);
  CHECK(k >= 0 && log.wheel_y[k] > 0.0f && (log.mods[k] & BK_FROM_TOUCH));
  k = find_type(&log, BK_CUSTOM_MAGNIFY);
  CHECK(k >= 0 && fabsf(log.magnify[k] - 1.0f) < 1e-3f); /* 60 px apart to 120 */
  CHECK(find_type(&log, BK_CUSTOM_PRESS) < 0);
  /* a trackpad turn */
  memset(&log, 0, sizeof(log));
  BK_rotate(ctx, (int)cx, (int)cy, 30.0f, 0);
  CHECK(log.n == 1 && log.type[0] == BK_CUSTOM_ROTATE && log.rotate[0] == 30.0f);

  /* the scrollbar strip at its right edge is a press, as is any place in the pointer mode */
  memset(&log, 0, sizeof(log));
  BK_touch(ctx, 3, BK_TOUCH_DOWN, (float)(r[0] + r[2] - 4), cy, 0, 0);
  CHECK(find_type(&log, BK_CUSTOM_PRESS) >= 0);
  BK_touch(ctx, 3, BK_TOUCH_UP, (float)(r[0] + r[2] - 4), cy, 0, 0);
  BK_custom_set_touch(node, BK_TOUCH_MODE_POINTER);
  memset(&log, 0, sizeof(log));
  BK_touch(ctx, 4, BK_TOUCH_DOWN, cx, cy, 0, 0);
  k = find_type(&log, BK_CUSTOM_PRESS);
  CHECK(k >= 0 && (log.mods[k] & BK_FROM_TOUCH));
  BK_touch(ctx, 4, BK_TOUCH_MOVE, cx + 40, cy, 0, 0);
  CHECK(log.type[log.n - 1] == BK_CUSTOM_MOTION);
  BK_touch(ctx, 4, BK_TOUCH_UP, cx + 40, cy, 0, 0);
  CHECK(find_type(&log, BK_CUSTOM_RELEASE) >= 0);

  BK_context_free(ctx);
  BK_node_free(col);
}

/* -- menus: a tap opens one, and the finger leaving does not close it ------------------------ */

static void test_menu(void)
{
  BKNode *col = BK_node_new("column");
  BKNode *menu = BK_node_new("menubutton");
  BK_set_str(menu, "text", "File");
  BKMenuSpec items[3];
  memset(items, 0, sizeof(items));
  const char *names[] = {"New", "Open", "Save"};
  for (int i = 0; i < 3; i++) {
    items[i].text = names[i];
    items[i].checked = -1;
    items[i].enabled = 1;
  }
  BK_menubutton_set_items(menu, items, 3);
  BK_node_add(col, menu, -1);
  BKContext *ctx = context(col);
  render(ctx);
  int x, y;
  centre(menu, &x, &y);
  BK_touch(ctx, 1, BK_TOUCH_DOWN, (float)x, (float)y, 0, 0);
  BK_touch(ctx, 1, BK_TOUCH_UP, (float)x, (float)y, 0, 0);
  render(ctx);
  CHECK(BK_popup_count(ctx) == 1);
  BK_context_free(ctx);
  BK_node_free(col);
}

int main(void)
{
  g_raster = BUI_raster_create(W, H);
  test_tap_and_pointer();
  test_scroll_and_fling();
  test_node_editor();
  test_custom();
  test_menu();
  BUI_raster_free(g_raster);
  printf(g_fail ? "touch: FAILED\n" : "touch: ok\n");
  return g_fail;
}
