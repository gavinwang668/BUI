/* The desktop shell (BK_shell_*, BK_tray_*, BK_notify, BK_window_set_progress): shell.c owns the
 * state and checks every call at the boundary; one backend per system talks to it:
 *   shell_win32.c  Shell_NotifyIconW, ITaskbarList3, SHAddToRecentDocs
 *   shell_dbus.c   StatusNotifierItem, com.canonical.dbusmenu, org.freedesktop.Notifications,
 *                  the Unity launcher entry (libdbus-1 loaded at run time), recently-used.xbel
 *   shell_cocoa.m  NSStatusItem, UNUserNotificationCenter, the Dock tile, NSDocumentController
 *   shell_off.c    BUI_SHELL off, or a system with none: the API, reporting nothing there
 * A backend gets only checked arguments: a live tray, a state in range, non-empty strings.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */
#pragma once

#include <stdbool.h>

#include "BUI_kit.h"

struct BKTray {
  BKTray *next;
  int uid;       /* > 0, unique in the process (Windows' icon uID, Linux's bus name) */
  char *icon;    /* NULL: the app's own */
  char *tooltip; /* never NULL */
  BKMenuSpec *menu;
  int nmenu;
  void *os; /* the backend's */
};

/* -- shell.c, for the backends -- */
extern BKTray *bk_shell_trays; /* every live tray, newest first */
/* What `icon` names: an image file (any size, fit within `size`) or a Blender icon, tinted `tint`
 * when it is a mono one; `icon` NULL or no such image: NULL. Free with BK_imbuf_free. */
BKImBuf *bk_shell_icon(const char *icon, int size, const float tint[4]);
void bk_shell_emit(BKTray *tray, int kind, void *ref, double a, double b);
/* Item `id` of a tray's menu (numbered from 1 in pre-order, as BK_EV_CALL reports) was chosen:
 * runs its `fn` or emits BK_EV_CALL. Separators, disabled items and unknown ids do nothing. */
void bk_shell_menu_chosen(BKTray *tray, int id);
/* Item `id` of `items`, NULL past the end. Menus are numbered from 1 in pre-order (an item, then
 * its submenu's items): a backend building one counts the same way. */
const BKMenuSpec *bk_shell_menu_item(const BKMenuSpec *items, int n, int id);
const char *bk_shell_app_id(void); /* BK_shell_set_app_id, NULL before it */

/* -- the backend -- */
enum { BK_TRAY_ICON = 1, BK_TRAY_TOOLTIP = 2, BK_TRAY_MENU = 4 }; /* what changed */
int bk_shell_os_features(void);
bool bk_shell_os_tray_add(BKTray *tray);
void bk_shell_os_tray_remove(BKTray *tray);
bool bk_shell_os_tray_update(BKTray *tray, int changed);
bool bk_shell_os_notify(int id, const char *title, const char *body, const char *icon);
bool bk_shell_os_progress(BKWindow *win, int state, float value);
bool bk_shell_os_add_recent(const char *path);
void bk_shell_os_set_app_id(const char *id);
