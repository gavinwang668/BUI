/* The desktop shell's API without a shell: built without BUI_SHELL, or for a system with no
 * backend (Android, WebAssembly). Every part is missing (BK_shell_features() is 0).
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#include "kit_intern.h"

int BK_shell_features(void)
{
  return 0;
}

void BK_shell_set_app_id(const char *id)
{
  (void)id;
}

void BK_shell_set_callbacks(const BKShellCallbacks *cb)
{
  (void)cb;
}

BKTray *BK_tray_new(const char *icon, const char *tooltip)
{
  (void)icon;
  (void)tooltip;
  return NULL;
}

void BK_tray_free(BKTray *tray)
{
  (void)tray;
}

int BK_tray_set_icon(BKTray *tray, const char *icon)
{
  (void)tray;
  (void)icon;
  return 0;
}

int BK_tray_set_tooltip(BKTray *tray, const char *tooltip)
{
  (void)tray;
  (void)tooltip;
  return 0;
}

int BK_tray_set_menu(BKTray *tray, const BKMenuSpec *items, int n)
{
  (void)tray;
  (void)items;
  (void)n;
  return 0;
}

int BK_notify(const char *title, const char *body, const char *icon)
{
  (void)title;
  (void)body;
  (void)icon;
  return 0;
}

int BK_window_set_progress(BKWindow *win, int state, float value)
{
  (void)win;
  (void)state;
  (void)value;
  return 0;
}

int BK_shell_add_recent(const char *path)
{
  (void)path;
  return 0;
}

int bk_shell_fd(void)
{
  return -1;
}

void bk_shell_poll(void) {}
