/* The desktop shell's state and its boundary (shell.h): every public call is checked here, and
 * the backend of this system (shell_win32.c, shell_dbus.c, shell_cocoa.m) does the rest.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#include <stdlib.h>
#include <string.h>

#include "kit_intern.h"
#include "shell.h"

BKTray *bk_shell_trays = NULL;
static BKShellCallbacks g_cb;
static char *g_app_id;
static int g_next_uid = 0;
static int g_next_notification = 0;

static bool has(int feature)
{
  return (BK_shell_features() & feature) != 0;
}

static bool is_tray(const BKTray *tray)
{
  for (const BKTray *t = bk_shell_trays; t; t = t->next) {
    if (t == tray) {
      return true;
    }
  }
  return false;
}

static char *dup_or_null(const char *s)
{
  return (s && s[0]) ? bk_strdup(s) : NULL;
}

int BK_shell_features(void)
{
  static int features = -1; /* asked once: the backend connects or loads what it needs */
  if (features < 0) {
    features = bk_shell_os_features();
  }
  return features;
}

void BK_shell_set_app_id(const char *id)
{
  if (!id || !id[0]) {
    return;
  }
  free(g_app_id);
  g_app_id = bk_strdup(id);
  bk_shell_os_set_app_id(g_app_id);
}

const char *bk_shell_app_id(void)
{
  return g_app_id;
}

void BK_shell_set_callbacks(const BKShellCallbacks *cb)
{
  if (cb) {
    g_cb = *cb;
  }
  else {
    memset(&g_cb, 0, sizeof(g_cb));
  }
}

void bk_shell_emit(BKTray *tray, int kind, void *ref, double a, double b)
{
  if (g_cb.emit) {
    g_cb.emit(g_cb.data, tray, kind, ref, a, b);
  }
}

BKImBuf *bk_shell_icon(const char *icon, int size, const float tint[4])
{
  if (!icon || !icon[0]) {
    return NULL;
  }
  BKImBuf *ib = bk_icon_image(icon, size, tint);
  if (!ib) {
    ib = BK_imbuf_load(icon, size, size);
  }
  int w = 0, h = 0;
  if (!ib || (BK_imbuf_size(ib, &w, &h) && w == size && h == size)) {
    return ib;
  }
  /* centered on a square: the systems take square icons (a Windows balloon refuses others) */
  uint32_t *px = (uint32_t *)calloc((size_t)size * (size_t)size, sizeof(uint32_t));
  if (px) {
    const uint32_t *src = BK_imbuf_pixels(ib, 0);
    const int x0 = (size - w) / 2, y0 = (size - h) / 2;
    for (int y = 0; y < h; y++) {
      memcpy(&px[(size_t)(y0 + y) * (size_t)size + (size_t)x0], &src[(size_t)y * (size_t)w],
             sizeof(uint32_t) * (size_t)w);
    }
  }
  BK_imbuf_free(ib);
  BKImBuf *square = px ? BK_imbuf_from_argb(px, size, size) : NULL;
  free(px);
  return square;
}

/* -- trays -- */

BKTray *BK_tray_new(const char *icon, const char *tooltip)
{
  if (!has(BK_SHELL_TRAY)) {
    return NULL;
  }
  BKTray *t = (BKTray *)calloc(1, sizeof(BKTray));
  if (!t) {
    return NULL;
  }
  t->uid = ++g_next_uid;
  t->icon = dup_or_null(icon);
  t->tooltip = bk_strdup(tooltip ? tooltip : "");
  if (!bk_shell_os_tray_add(t)) {
    free(t->icon);
    free(t->tooltip);
    free(t);
    return NULL;
  }
  t->next = bk_shell_trays;
  bk_shell_trays = t;
  return t;
}

void BK_tray_free(BKTray *tray)
{
  if (!is_tray(tray)) {
    return;
  }
  BKTray **p = &bk_shell_trays;
  while (*p != tray) {
    p = &(*p)->next;
  }
  *p = tray->next;
  bk_shell_os_tray_remove(tray);
  bk_menuspec_free(tray->menu, tray->nmenu);
  free(tray->icon);
  free(tray->tooltip);
  free(tray);
}

int BK_tray_set_icon(BKTray *tray, const char *icon)
{
  if (!is_tray(tray)) {
    return 0;
  }
  free(tray->icon);
  tray->icon = dup_or_null(icon);
  return bk_shell_os_tray_update(tray, BK_TRAY_ICON);
}

int BK_tray_set_tooltip(BKTray *tray, const char *tooltip)
{
  if (!is_tray(tray)) {
    return 0;
  }
  free(tray->tooltip);
  tray->tooltip = bk_strdup(tooltip ? tooltip : "");
  return bk_shell_os_tray_update(tray, BK_TRAY_TOOLTIP);
}

int BK_tray_set_menu(BKTray *tray, const BKMenuSpec *items, int n)
{
  if (!is_tray(tray) || n < 0 || (n > 0 && !items)) {
    return 0;
  }
  BKMenuSpec *menu = bk_menuspec_copy(items, n); /* before the free: `items` may be the old menu */
  bk_menuspec_free(tray->menu, tray->nmenu);
  tray->menu = menu;
  tray->nmenu = menu ? n : 0;
  return bk_shell_os_tray_update(tray, BK_TRAY_MENU);
}

/* Pre-order from 1: `*id` counts down to the item. */
static const BKMenuSpec *menu_find(const BKMenuSpec *items, int n, int *id)
{
  for (int i = 0; i < n; i++) {
    if (--*id == 0) {
      return &items[i];
    }
    const BKMenuSpec *in = menu_find(items[i].sub, items[i].nsub, id);
    if (in) {
      return in;
    }
  }
  return NULL;
}

const BKMenuSpec *bk_shell_menu_item(const BKMenuSpec *items, int n, int id)
{
  return id > 0 ? menu_find(items, n, &id) : NULL;
}

void bk_shell_menu_chosen(BKTray *tray, int id)
{
  const BKMenuSpec *it = bk_shell_menu_item(tray->menu, tray->nmenu, id);
  if (!it || bk_menuspec_is_sep(it) || !it->enabled || it->nsub > 0) {
    return;
  }
  if (it->fn) {
    it->fn(it->data);
    return;
  }
  void *ref = it->ref; /* the listener may set another menu, which releases this one */
  bk_retain(ref);
  bk_shell_emit(tray, BK_EV_CALL, ref, id, 0);
  bk_release(ref);
}

/* -- notifications, progress, recent files -- */

int BK_notify(const char *title, const char *body, const char *icon)
{
  if (!has(BK_SHELL_NOTIFY) || ((!title || !title[0]) && (!body || !body[0]))) {
    return 0;
  }
  const int id = ++g_next_notification;
  return bk_shell_os_notify(id, title ? title : "", body ? body : "", (icon && icon[0]) ? icon : NULL) ?
             id :
             0;
}

int BK_window_set_progress(BKWindow *win, int state, float value)
{
  if (!win || state < BK_PROGRESS_NONE || state > BK_PROGRESS_ERROR || !has(BK_SHELL_PROGRESS)) {
    return 0;
  }
  value = value != value ? 0.0f : BK_CLAMP(value, 0.0f, 1.0f); /* NaN: 0 */
  return bk_shell_os_progress(win, state, value);
}

int BK_shell_add_recent(const char *path)
{
  if (!path || !path[0] || !has(BK_SHELL_RECENT)) {
    return 0;
  }
  return bk_shell_os_add_recent(path);
}
