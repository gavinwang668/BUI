/* The desktop shell (kit/intern/shell): a tray icon with a menu, a notification, a window's
 * taskbar progress and a recent file, through the real system calls; checks the return codes,
 * that the system has the objects (Windows: the icon in the notification area, the shell's
 * window, the recent file's shortcut), the checks at the boundary, and the events (the system's
 * messages posted to the shell's window, as the notification area sends them).
 * Exits 77 (skipped) when this build or system has no tray (BUI_SHELL off, no desktop session).
 * BUI_SHELL_TEST_HOLD=<ms> keeps the icon and the notification up that long (screenshots).
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "BUI_kit.h"
#include "shell.h" /* bk_shell_menu_*: the menu's numbering and choice without a mouse */

#ifdef _WIN32
#  define WIN32_LEAN_AND_MEAN
#  include <windows.h>
#  include <shellapi.h>
#  define WM_BK_TRAY (WM_APP + 20) /* shell_win32.c */
#else
#  include <unistd.h>
void bk_shell_poll(void); /* kit_intern.h: the window loop's call */
#endif

static int g_fail = 0;
#define CHECK(c) \
  do { \
    if (!(c)) { \
      printf("FAIL %s:%d: %s\n", __FILE__, __LINE__, #c); \
      g_fail++; \
    } \
  } while (0)

typedef struct Ev {
  BKTray *tray;
  int kind;
  void *ref;
  double a;
} Ev;
static Ev g_ev[32];
static int g_nev = 0;
static int g_quit_called = 0;

static void emit(void *data, BKTray *tray, int kind, void *ref, double a, double b)
{
  (void)data;
  (void)b;
  if (g_nev < 32) {
    g_ev[g_nev++] = (Ev){tray, kind, ref, a};
  }
}

static void quit_fn(void *data)
{
  g_quit_called += data == (void *)&g_quit_called;
}

static void pump_ms(int ms)
{
#ifdef _WIN32
  const DWORD end = GetTickCount() + (DWORD)ms;
  do {
    BK_window_pump();
    Sleep(10);
  } while ((int)(end - GetTickCount()) > 0);
#else
  static int windows = -1;
  if (windows < 0) {
    windows = BK_window_supported();
  }
  for (int i = 0; i < ms / 10; i++) {
    if (windows) {
      BK_window_pump(); /* its loop reads the shell's D-Bus too */
    }
    else {
      bk_shell_poll();
    }
    usleep(10000);
  }
#endif
}

static void test_menu(BKTray *tray)
{
  static const BKMenuSpec more[] = {
      {.text = "A", .checked = -1, .enabled = 1, .ref = (void *)0xA},
      {.text = "B", .checked = -1, .enabled = 0, .ref = (void *)0xB},
  };
  const BKMenuSpec items[] = {
      {.text = "Open", .checked = -1, .enabled = 1, .ref = (void *)0x1},
      {.text = "-"},
      {.text = "Check", .checked = 1, .enabled = 1, .ref = (void *)0x3},
      {.text = "More", .checked = -1, .enabled = 1, .sub = more, .nsub = 2},
      {.text = "Quit", .checked = -1, .enabled = 1, .fn = quit_fn, .data = &g_quit_called},
  };
  CHECK(BK_tray_set_menu(tray, items, 5));
  CHECK(!BK_tray_set_menu(tray, NULL, 3)); /* items missing: refused at the boundary */
  CHECK(!BK_tray_set_menu(tray, items, -1));
  /* pre-order from 1: Open 1, separator 2, Check 3, More 4, A 5, B 6, Quit 7 */
  const BKMenuSpec *a = bk_shell_menu_item(tray->menu, tray->nmenu, 5);
  CHECK(a && strcmp(a->text, "A") == 0);
  CHECK(bk_shell_menu_item(tray->menu, tray->nmenu, 8) == NULL);
  CHECK(bk_shell_menu_item(tray->menu, tray->nmenu, 0) == NULL);
  g_nev = 0;
  bk_shell_menu_chosen(tray, 5);
  CHECK(g_nev == 1 && g_ev[0].kind == BK_EV_CALL && g_ev[0].ref == (void *)0xA && g_ev[0].a == 5 &&
        g_ev[0].tray == tray);
  bk_shell_menu_chosen(tray, 2); /* separator */
  bk_shell_menu_chosen(tray, 4); /* a submenu's parent */
  bk_shell_menu_chosen(tray, 6); /* disabled */
  bk_shell_menu_chosen(tray, 99);
  CHECK(g_nev == 1);
  bk_shell_menu_chosen(tray, 7);
  CHECK(g_quit_called == 1 && g_nev == 1);
}

#ifdef _WIN32
/* Whether the notification area has icon `uid` of the shell's window. */
static int icon_exists(HWND hwnd, UINT uid)
{
  typedef HRESULT(WINAPI * GetRectFn)(const NOTIFYICONIDENTIFIER *, RECT *);
  GetRectFn get_rect = (GetRectFn)(void *)GetProcAddress(LoadLibraryW(L"shell32.dll"),
                                                         "Shell_NotifyIconGetRect");
  NOTIFYICONIDENTIFIER id = {sizeof(id), hwnd, uid};
  RECT r;
  return get_rect && SUCCEEDED(get_rect(&id, &r));
}
#endif

int main(void)
{
  const int features = BK_shell_features();
  printf("shell features: tray %d notify %d progress %d recent %d\n",
         !!(features & BK_SHELL_TRAY), !!(features & BK_SHELL_NOTIFY),
         !!(features & BK_SHELL_PROGRESS), !!(features & BK_SHELL_RECENT));
  BKShellCallbacks cb = {NULL, emit};
  BK_shell_set_callbacks(&cb);
  const char *hold_env = getenv("BUI_SHELL_TEST_HOLD");
  const int hold = hold_env ? atoi(hold_env) : 0;

  /* boundary checks hold with or without a shell */
  CHECK(BK_window_set_progress(NULL, BK_PROGRESS_NORMAL, 0.5f) == 0);
  CHECK(BK_shell_add_recent(NULL) == 0 && BK_shell_add_recent("") == 0);
  CHECK(BK_notify(NULL, NULL, NULL) == 0 && BK_notify("", "", NULL) == 0);
  CHECK(BK_tray_set_tooltip(NULL, "x") == 0 && BK_tray_set_icon(NULL, NULL) == 0);
  BK_tray_free(NULL);

  BKTray *tray = BK_tray_new("BLENDER", "BUI shell test");
  if (!tray) {
    printf("no tray here (BUI_SHELL off, or no desktop session): skipped\n");
    return 77;
  }
  CHECK(BK_tray_set_tooltip(tray, "BUI shell test: tooltip"));
  CHECK(BK_tray_set_icon(tray, BUI_TEST_IMAGES "/quad.png"));
  CHECK(!BK_tray_set_icon(tray, BUI_TEST_IMAGES "/no-such-image.png"));
  CHECK(BK_tray_set_icon(tray, "BLENDER"));
  test_menu(tray);

#ifdef _WIN32
  HWND shell = FindWindowW(L"BUIKitShell", NULL);
  CHECK(shell != NULL);
  CHECK(icon_exists(shell, (UINT)tray->uid));
  /* the notification area's messages (version 4: the event and the icon in lParam) */
  g_nev = 0;
  PostMessageW(shell, WM_BK_TRAY, MAKEWPARAM(10, 10), MAKELPARAM(NIN_SELECT, tray->uid));
  PostMessageW(shell, WM_BK_TRAY, MAKEWPARAM(10, 10), MAKELPARAM(WM_LBUTTONDBLCLK, tray->uid));
  PostMessageW(shell, WM_BK_TRAY, MAKEWPARAM(10, 10), MAKELPARAM(WM_MBUTTONUP, tray->uid));
  pump_ms(50);
  CHECK(g_nev == 3 && g_ev[0].kind == BK_EV_CLICK && g_ev[0].a == 1 && g_ev[0].tray == tray &&
        g_ev[1].kind == BK_EV_DOUBLE && g_ev[2].kind == BK_EV_CLICK && g_ev[2].a == 2);
#endif

  /* a notification (on the tray icon) */
  const int note = BK_notify("BUI", "A notification from the shell test", NULL);
  CHECK(note > 0);
  const int note2 = BK_notify("BUI", "With an icon", "BLENDER");
  CHECK(note2 > note);
#ifdef _WIN32
  g_nev = 0;
  PostMessageW(shell, WM_BK_TRAY, 0, MAKELPARAM(NIN_BALLOONUSERCLICK, tray->uid));
  pump_ms(50);
  CHECK(g_nev == 1 && g_ev[0].kind == BK_EV_ACTIVATE && g_ev[0].tray == NULL &&
        g_ev[0].a == note2);
#endif

#ifndef _WIN32
  /* tests/shell_dbus_host.py plays the desktop: it reads the item and its menu, clicks the icon
   * and the menu's "A" (id 5), and clicks the first notification */
  if (getenv("BUI_SHELL_TEST_HOST")) {
    g_nev = 0;
    int click = 0, call = 0, activate = 0;
    for (int t = 0; t < 300 && !(click && call && activate); t++) {
      pump_ms(10);
      for (int i = 0; i < g_nev; i++) {
        click |= g_ev[i].kind == BK_EV_CLICK && g_ev[i].a == 1 && g_ev[i].tray == tray;
        call |= g_ev[i].kind == BK_EV_CALL && g_ev[i].a == 5 && g_ev[i].ref == (void *)0xA;
        activate |= g_ev[i].kind == BK_EV_ACTIVATE && g_ev[i].a == note && !g_ev[i].tray;
      }
    }
    CHECK(click);
    CHECK(call);
    CHECK(activate);
  }
#endif

  /* a window's taskbar progress */
  if (BK_window_supported()) {
    BKNode *root = BK_node_new("column");
    BKContext *ctx = BK_context_new(root);
    BKWindow *win = BK_window_new(ctx, "BUI shell test", 320, 200);
    BK_window_show(win);
    pump_ms(300); /* the taskbar button exists once the window shows */
    CHECK(BK_window_set_progress(win, BK_PROGRESS_NORMAL, 0.25f) == 1);
    CHECK(BK_window_set_progress(win, BK_PROGRESS_PAUSED, 0.5f) == 1);
    CHECK(BK_window_set_progress(win, BK_PROGRESS_ERROR, 2.0f) == 1); /* clamped */
    CHECK(BK_window_set_progress(win, BK_PROGRESS_INDETERMINATE, 0.0f) == 1);
    CHECK(BK_window_set_progress(win, 99, 0.5f) == 0);  /* no such state */
    CHECK(BK_window_set_progress(win, -1, 0.5f) == 0);
    CHECK(BK_window_set_progress(win, BK_PROGRESS_NORMAL, 0.6f) == 1);
    if (hold > 0) {
      pump_ms(hold);
    }
    CHECK(BK_window_set_progress(win, BK_PROGRESS_NONE, 0.0f) == 1);
    BK_window_free(win);
    BK_context_free(ctx);
    BK_node_free(root);
  }
  else if (hold > 0) {
    pump_ms(hold);
  }

  /* a recent file, named anew each run: Windows makes no shortcut again for a name it just had
   * (nor for files under %TEMP%) */
  char name[64], recent[1024];
  snprintf(name, sizeof(name), "bui_shell_test_%ld.txt", (long)time(NULL));
  snprintf(recent, sizeof(recent), "%s/%s", BUI_TEST_TMP, name);
  FILE *f = fopen(recent, "w");
  CHECK(f != NULL);
  if (f) {
    fputs("recent\n", f);
    fclose(f);
  }
  CHECK(BK_shell_add_recent(recent) == 1);
  CHECK(BK_shell_add_recent(BUI_TEST_TMP "/no-such-file.txt") == 0);
#ifdef _WIN32
  /* Explorer's recent items get a shortcut: checked, then removed again */
  char lnk[MAX_PATH];
  snprintf(lnk, sizeof(lnk), "%s\\Microsoft\\Windows\\Recent\\%s.lnk",
           getenv("APPDATA") ? getenv("APPDATA") : ".", name);
  int found = 0;
  for (int i = 0; i < 400 && !found; i++) { /* Explorer writes it soon after, not at once */
    found = GetFileAttributesA(lnk) != INVALID_FILE_ATTRIBUTES;
    if (!found) {
      Sleep(20);
    }
  }
  CHECK(found);
  DeleteFileA(lnk);
#endif
  remove(recent);

#ifdef _WIN32
  const UINT uid = (UINT)tray->uid;
#endif
  BK_tray_free(tray);
#ifdef _WIN32
  CHECK(!icon_exists(shell, uid));
#endif
  CHECK(BK_tray_set_tooltip(tray, "freed") == 0); /* a freed tray is refused, not used */

  /* a notification without a tray: the shell's own icon shows while it does */
  const int note3 = BK_notify("BUI", "Without a tray icon", NULL);
  CHECK(note3 > 0);
#ifdef _WIN32
  CHECK(icon_exists(shell, 0));
  g_nev = 0;
  PostMessageW(shell, WM_BK_TRAY, 0, MAKELPARAM(NIN_BALLOONUSERCLICK, 0));
  pump_ms(50);
  CHECK(g_nev == 1 && g_ev[0].kind == BK_EV_ACTIVATE && g_ev[0].a == note3);
  CHECK(!icon_exists(shell, 0)); /* gone with its notification */
#endif

  printf("%s (%d failed)\n", g_fail ? "FAILED" : "ok", g_fail);
  return g_fail ? 1 : 0;
}
