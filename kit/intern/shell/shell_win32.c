/* The desktop shell on Windows (shell.h).
 *
 *  tray icons      Shell_NotifyIconW (NOTIFYICON_VERSION_4) on a hidden window of the shell's,
 *                  menus with TrackPopupMenuEx, icons added again when Explorer restarts
 *                  ("TaskbarCreated"), as tauri's tray-icon
 *  notifications   the balloon of a tray icon (NIF_INFO), which Windows 10 and 11 show as a
 *                  toast and report clicked (NIN_BALLOONUSERCLICK); without a tray icon, one of
 *                  the shell's own shows while it does
 *  progress        ITaskbarList3, as GHOST_WindowWin32::setProgressBar / endProgressBar
 *  recent files    SHAddToRecentDocs, as GHOST_SystemPathsWin32::addToSystemRecentFiles
 *
 * shell32, ole32 and advapi32 are loaded at run time (as the window host loads dwmapi): linking
 * the kit needs no more libraries.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#ifdef _WIN32

#  ifndef UNICODE
#    define UNICODE
#  endif
#  define COBJMACROS
#  include <windows.h>
#  include <objbase.h>
#  include <shellapi.h>
#  include <shlobj.h>
#  include <shobjidl.h>
#  include <stdlib.h>
#  include <string.h>

#  include "kit_intern.h"
#  include "shell.h"

#  define WM_BK_TRAY (WM_APP + 20)
#  define OWN_ICON 0 /* the uID of the shell's own icon (notifications without a tray) */

typedef BOOL(WINAPI *NotifyIconFn)(DWORD, PNOTIFYICONDATAW);
typedef void(WINAPI *AddRecentFn)(UINT, LPCVOID);
typedef HRESULT(WINAPI *SetAppIdFn)(PCWSTR);
typedef HRESULT(WINAPI *ItemFromPathFn)(PCWSTR, IBindCtx *, REFIID, void **);
typedef UINT(WINAPI *ExtractIconsFn)(LPCWSTR, int, HICON *, HICON *, UINT);
typedef HRESULT(WINAPI *CoInitializeExFn)(LPVOID, DWORD);
typedef HRESULT(WINAPI *CoCreateInstanceFn)(REFCLSID, LPUNKNOWN, DWORD, REFIID, LPVOID *);
typedef LSTATUS(WINAPI *RegGetValueFn)(HKEY, LPCWSTR, LPCWSTR, DWORD, LPDWORD, PVOID, LPDWORD);

/* Not linked from uuid.lib: the kit links no more than user32, gdi32 and imm32. */
static const CLSID k_clsid_taskbar = {
    0x56fdf344, 0xfd6d, 0x11d0, {0x95, 0x8a, 0x00, 0x60, 0x97, 0xc9, 0xa0, 0x90}};
static const IID k_iid_taskbar3 = {
    0xea1afb91, 0x9e28, 0x4b86, {0x90, 0xe9, 0x9e, 0x9f, 0x8a, 0x5e, 0xef, 0xaf}};
static const IID k_iid_shell_item = {
    0x43826d1e, 0xe718, 0x42ee, {0xbc, 0x55, 0xa1, 0xe2, 0x61, 0xc3, 0x7b, 0xfe}};

static struct {
  bool loaded;
  NotifyIconFn notify_icon;
  AddRecentFn add_recent;
  SetAppIdFn set_app_id;
  ItemFromPathFn item_from_path;
  ExtractIconsFn extract_icons;
  CoInitializeExFn co_init;
  CoCreateInstanceFn co_create;
  RegGetValueFn reg_get;
  HWND hwnd;             /* receives the icons' messages */
  UINT taskbar_created;  /* Explorer started again: add the icons again */
  ITaskbarList3 *bar;    /* NULL: not made yet, or not available */
  bool bar_tried;
  HICON own_icon;        /* the shell's own icon while it shows a notification, else NULL */
  int own_notification;
} S;

/* One tray's Windows side: its icon (Shell_NotifyIcon keeps using the handle) and the
 * notification its balloon shows. */
typedef struct TrayWin32 {
  HICON icon;
  int notification;
} TrayWin32;

static FARPROC sym(const wchar_t *dll, const char *name)
{
  HMODULE m = LoadLibraryW(dll);
  return m ? GetProcAddress(m, name) : NULL;
}

static void load(void)
{
  if (S.loaded) {
    return;
  }
  S.loaded = true;
  S.notify_icon = (NotifyIconFn)(void *)sym(L"shell32.dll", "Shell_NotifyIconW");
  S.add_recent = (AddRecentFn)(void *)sym(L"shell32.dll", "SHAddToRecentDocs");
  S.set_app_id = (SetAppIdFn)(void *)sym(L"shell32.dll", "SetCurrentProcessExplicitAppUserModelID");
  S.item_from_path = (ItemFromPathFn)(void *)sym(L"shell32.dll", "SHCreateItemFromParsingName");
  S.extract_icons = (ExtractIconsFn)(void *)sym(L"shell32.dll", "ExtractIconExW");
  S.co_init = (CoInitializeExFn)(void *)sym(L"ole32.dll", "CoInitializeEx");
  S.co_create = (CoCreateInstanceFn)(void *)sym(L"ole32.dll", "CoCreateInstance");
  S.reg_get = (RegGetValueFn)(void *)sym(L"advapi32.dll", "RegGetValueW");
}

/* COM on this (the UI) thread, as GHOST_SystemWin32's OleInitialize: once, kept. */
static bool com(void)
{
  static int done = 0;
  if (!done) {
    const HRESULT hr = S.co_init ? S.co_init(NULL, COINIT_APARTMENTTHREADED) : E_FAIL;
    done = (SUCCEEDED(hr) || hr == RPC_E_CHANGED_MODE) ? 1 : -1;
  }
  return done > 0;
}

static void copy_wide(wchar_t *dst, size_t cap, const char *s)
{
  wchar_t *w = bk_utf8_to_wide(s);
  dst[0] = 0;
  if (w) {
    wcsncpy(dst, w, cap - 1);
    dst[cap - 1] = 0;
    free(w);
  }
}

/* -- icons -- */

/* Mono Blender icons in the taskbar's text color: dark on a light taskbar, else white. */
static void tint(float r_color[4])
{
  DWORD light = 0, size = sizeof(light);
  const bool is_light = S.reg_get &&
                        S.reg_get(HKEY_CURRENT_USER,
                                  L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",
                                  L"SystemUsesLightTheme", RRF_RT_REG_DWORD, NULL, &light,
                                  &size) == ERROR_SUCCESS &&
                        light;
  const float v = is_light ? 0.0f : 1.0f;
  r_color[0] = r_color[1] = r_color[2] = v;
  r_color[3] = 1.0f;
}

static HICON icon_from(const BKImBuf *ib)
{
  int w = 0, h = 0;
  BK_imbuf_size(ib, &w, &h);
  unsigned char *rgba = (unsigned char *)malloc((size_t)w * (size_t)h * 4);
  if (!rgba || !BK_imbuf_rgba(ib, 0, rgba)) {
    free(rgba);
    return NULL;
  }
  BITMAPV5HEADER bi = {0};
  bi.bV5Size = sizeof(bi);
  bi.bV5Width = w;
  bi.bV5Height = -h; /* top-down */
  bi.bV5Planes = 1;
  bi.bV5BitCount = 32;
  bi.bV5Compression = BI_BITFIELDS;
  bi.bV5RedMask = 0x00ff0000;
  bi.bV5GreenMask = 0x0000ff00;
  bi.bV5BlueMask = 0x000000ff;
  bi.bV5AlphaMask = 0xff000000;
  void *bits = NULL;
  HDC dc = GetDC(NULL);
  HBITMAP color = CreateDIBSection(dc, (BITMAPINFO *)&bi, DIB_RGB_COLORS, &bits, NULL, 0);
  ReleaseDC(NULL, dc);
  HBITMAP mask = CreateBitmap(w, h, 1, 1, NULL);
  HICON icon = NULL;
  if (color && mask && bits) {
    uint32_t *px = (uint32_t *)bits;
    for (int i = 0; i < w * h; i++) { /* straight alpha, BGRA in memory */
      const unsigned char *p = &rgba[i * 4];
      px[i] = ((uint32_t)p[3] << 24) | ((uint32_t)p[0] << 16) | ((uint32_t)p[1] << 8) | p[2];
    }
    ICONINFO ii = {TRUE, 0, 0, mask, color};
    icon = CreateIconIndirect(&ii);
  }
  if (color) {
    DeleteObject(color);
  }
  if (mask) {
    DeleteObject(mask);
  }
  free(rgba);
  return icon;
}

/* The executable's own icon (its first icon resource), else Windows' application icon. */
static HICON app_icon(void)
{
  wchar_t exe[MAX_PATH];
  HICON icon = NULL; /* not `small`: rpcndr.h defines that */
  if (S.extract_icons && GetModuleFileNameW(NULL, exe, MAX_PATH) &&
      S.extract_icons(exe, 0, NULL, &icon, 1) && icon)
  {
    return icon;
  }
  return CopyIcon(LoadIconW(NULL, IDI_APPLICATION));
}

/* `icon` at the size of the notification area's icons (or `large`: a toast's); NULL: none. */
static HICON load_icon(const char *icon, bool large)
{
  float c[4];
  tint(c);
  const int size = GetSystemMetrics(large ? SM_CXICON : SM_CXSMICON);
  BKImBuf *ib = bk_shell_icon(icon, size, c);
  HICON h = ib ? icon_from(ib) : NULL;
  BK_imbuf_free(ib);
  return h;
}

/* -- the shell's window and its icons -- */

static TrayWin32 *os_of(const BKTray *t)
{
  return (TrayWin32 *)t->os;
}

static BKTray *tray_of(int uid)
{
  for (BKTray *t = bk_shell_trays; t; t = t->next) {
    if (t->uid == uid) {
      return t;
    }
  }
  return NULL;
}

static NOTIFYICONDATAW icon_data(UINT uid, UINT flags)
{
  NOTIFYICONDATAW d;
  memset(&d, 0, sizeof(d));
  d.cbSize = sizeof(d);
  d.hWnd = S.hwnd;
  d.uID = uid;
  d.uFlags = flags;
  return d;
}

/* NIM_ADD and the version-4 messages. */
static bool icon_add(UINT uid, HICON icon, const char *tooltip)
{
  NOTIFYICONDATAW d = icon_data(uid, NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP);
  d.uCallbackMessage = WM_BK_TRAY;
  d.hIcon = icon;
  copy_wide(d.szTip, sizeof(d.szTip) / sizeof(d.szTip[0]), tooltip);
  if (!S.notify_icon(NIM_ADD, &d)) {
    return false;
  }
  d.uVersion = NOTIFYICON_VERSION_4;
  S.notify_icon(NIM_SETVERSION, &d);
  return true;
}

static void icon_delete(UINT uid)
{
  NOTIFYICONDATAW d = icon_data(uid, 0);
  S.notify_icon(NIM_DELETE, &d);
}

static void own_icon_end(void)
{
  if (S.own_icon) {
    icon_delete(OWN_ICON);
    DestroyIcon(S.own_icon);
    S.own_icon = NULL;
    S.own_notification = 0;
  }
}

/* A menu's items from `*id` on, numbered as bk_shell_menu_item does. */
static HMENU build_menu(const BKMenuSpec *items, int n, int *id)
{
  HMENU m = CreatePopupMenu();
  for (int i = 0; i < n; i++) {
    const BKMenuSpec *it = &items[i];
    const int item_id = ++*id;
    if (bk_menuspec_is_sep(it)) {
      AppendMenuW(m, MF_SEPARATOR, 0, NULL);
      continue;
    }
    wchar_t *text = bk_utf8_to_wide(it->text);
    UINT flags = MF_STRING | (it->enabled ? 0 : MF_GRAYED) | (it->checked > 0 ? MF_CHECKED : 0);
    if (it->nsub > 0) {
      HMENU sub = build_menu(it->sub, it->nsub, id);
      AppendMenuW(m, flags | MF_POPUP, (UINT_PTR)sub, text);
    }
    else {
      AppendMenuW(m, flags, (UINT_PTR)item_id, text);
    }
    free(text);
  }
  return m;
}

/* The menu at (x, y), screen pixels (KB135788: the window comes forward first, and a message
 * after, so the menu closes when the user clicks elsewhere). */
static void show_menu(BKTray *t, int x, int y)
{
  int id = 0;
  HMENU m = build_menu(t->menu, t->nmenu, &id);
  const int uid = t->uid;
  SetForegroundWindow(S.hwnd);
  const UINT align = GetSystemMetrics(SM_MENUDROPALIGNMENT) ? TPM_RIGHTALIGN : TPM_LEFTALIGN;
  const int chosen = (int)TrackPopupMenuEx(
      m, TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON | align | TPM_BOTTOMALIGN, x, y, S.hwnd,
      NULL);
  PostMessageW(S.hwnd, WM_NULL, 0, 0);
  DestroyMenu(m);
  t = tray_of(uid); /* the menu's loop ran the app: the tray may be gone */
  if (t && chosen > 0) {
    bk_shell_menu_chosen(t, chosen);
  }
}

static void tray_message(UINT event, int uid, int x, int y)
{
  if (uid == OWN_ICON) {
    if (event == NIN_BALLOONUSERCLICK) {
      const int id = S.own_notification;
      own_icon_end();
      bk_shell_emit(NULL, BK_EV_ACTIVATE, NULL, id, 0);
    }
    else if (event == NIN_BALLOONTIMEOUT || event == NIN_BALLOONHIDE) {
      own_icon_end();
    }
    return;
  }
  BKTray *t = tray_of(uid);
  if (!t) {
    return;
  }
  switch (event) {
    case NIN_SELECT:
    case NIN_KEYSELECT:
      bk_shell_emit(t, BK_EV_CLICK, NULL, 1, 0);
      break;
    case WM_MBUTTONUP:
      bk_shell_emit(t, BK_EV_CLICK, NULL, 2, 0);
      break;
    case WM_LBUTTONDBLCLK:
      bk_shell_emit(t, BK_EV_DOUBLE, NULL, 1, 0);
      break;
    case WM_CONTEXTMENU:
      if (t->nmenu > 0) {
        show_menu(t, x, y);
      }
      else {
        bk_shell_emit(t, BK_EV_CLICK, NULL, 3, 0);
      }
      break;
    case NIN_BALLOONUSERCLICK: {
      const int id = os_of(t)->notification;
      os_of(t)->notification = 0;
      bk_shell_emit(NULL, BK_EV_ACTIVATE, NULL, id, 0);
      break;
    }
    case NIN_BALLOONTIMEOUT:
    case NIN_BALLOONHIDE:
      os_of(t)->notification = 0;
      break;
  }
}

static LRESULT CALLBACK shell_proc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp)
{
  if (msg == WM_BK_TRAY) {
    /* version 4: the event and the icon in lParam, the anchor point in wParam */
    tray_message(LOWORD(lp), HIWORD(lp), (short)LOWORD(wp), (short)HIWORD(wp));
    return 0;
  }
  if (msg == S.taskbar_created && msg) {
    for (BKTray *t = bk_shell_trays; t; t = t->next) {
      icon_add((UINT)t->uid, os_of(t)->icon, t->tooltip);
    }
    if (S.own_icon) {
      icon_add(OWN_ICON, S.own_icon, "");
    }
    return 0;
  }
  return DefWindowProcW(hwnd, msg, wp, lp);
}

/* The hidden window the icons talk to (a top-level one: message-only windows don't get the
 * broadcast "TaskbarCreated"), on the UI thread. */
static bool shell_window(void)
{
  if (S.hwnd) {
    return true;
  }
  load();
  if (!S.notify_icon) {
    return false;
  }
  WNDCLASSW wc = {0};
  wc.lpfnWndProc = shell_proc;
  wc.hInstance = GetModuleHandleW(NULL);
  wc.lpszClassName = L"BUIKitShell";
  RegisterClassW(&wc); /* fails harmlessly when already registered */
  S.hwnd = CreateWindowExW(WS_EX_TOOLWINDOW, L"BUIKitShell", L"", WS_POPUP, 0, 0, 0, 0, NULL,
                           NULL, wc.hInstance, NULL);
  S.taskbar_created = RegisterWindowMessageW(L"TaskbarCreated");
  return S.hwnd != NULL;
}

/* -- the backend -- */

int bk_shell_os_features(void)
{
  load();
  int f = 0;
  if (S.notify_icon) {
    f |= BK_SHELL_TRAY | BK_SHELL_NOTIFY;
  }
  if (S.co_init && S.co_create) {
    f |= BK_SHELL_PROGRESS;
  }
  if (S.add_recent) {
    f |= BK_SHELL_RECENT;
  }
  return f;
}

void bk_shell_os_set_app_id(const char *id)
{
  load();
  wchar_t *w = bk_utf8_to_wide(id);
  if (w && S.set_app_id) {
    S.set_app_id(w);
  }
  free(w);
}

bool bk_shell_os_tray_add(BKTray *t)
{
  TrayWin32 *os = (TrayWin32 *)calloc(1, sizeof(TrayWin32));
  if (!os || !shell_window()) {
    free(os);
    return false;
  }
  os->icon = load_icon(t->icon, false);
  if (!os->icon) {
    os->icon = app_icon();
  }
  if (!icon_add((UINT)t->uid, os->icon, t->tooltip)) {
    DestroyIcon(os->icon);
    free(os);
    return false;
  }
  t->os = os;
  return true;
}

void bk_shell_os_tray_remove(BKTray *t)
{
  icon_delete((UINT)t->uid);
  DestroyIcon(os_of(t)->icon);
  free(t->os);
  t->os = NULL;
}

bool bk_shell_os_tray_update(BKTray *t, int changed)
{
  TrayWin32 *os = os_of(t);
  NOTIFYICONDATAW d = icon_data((UINT)t->uid, 0);
  bool ok = true;
  if (changed & BK_TRAY_ICON) {
    HICON icon = load_icon(t->icon, false);
    ok = icon || !t->icon; /* a file that isn't an image keeps the old icon */
    if (ok) {
      DestroyIcon(os->icon);
      os->icon = icon ? icon : app_icon();
      d.uFlags |= NIF_ICON;
      d.hIcon = os->icon;
    }
  }
  if (changed & BK_TRAY_TOOLTIP) {
    d.uFlags |= NIF_TIP | NIF_SHOWTIP;
    copy_wide(d.szTip, sizeof(d.szTip) / sizeof(d.szTip[0]), t->tooltip);
  }
  if (d.uFlags) {
    ok = S.notify_icon(NIM_MODIFY, &d) && ok;
  }
  return ok; /* BK_TRAY_MENU: built when it opens */
}

bool bk_shell_os_notify(int id, const char *title, const char *body, const char *icon)
{
  if (!shell_window()) {
    return false;
  }
  BKTray *t = bk_shell_trays;
  UINT uid = t ? (UINT)t->uid : OWN_ICON;
  if (!t && !S.own_icon) {
    HICON own = app_icon();
    if (!icon_add(OWN_ICON, own, "")) {
      DestroyIcon(own);
      return false;
    }
    S.own_icon = own;
  }
  NOTIFYICONDATAW d = icon_data(uid, NIF_INFO);
  copy_wide(d.szInfoTitle, sizeof(d.szInfoTitle) / sizeof(d.szInfoTitle[0]), title);
  copy_wide(d.szInfo, sizeof(d.szInfo) / sizeof(d.szInfo[0]), body[0] ? body : " ");
  HICON big = icon ? load_icon(icon, true) : NULL;
  d.dwInfoFlags = NIIF_RESPECT_QUIET_TIME | (big ? NIIF_USER | NIIF_LARGE_ICON : NIIF_NONE);
  d.hBalloonIcon = big;
  const bool ok = S.notify_icon(NIM_MODIFY, &d) != FALSE;
  if (big) {
    DestroyIcon(big); /* the shell copied it */
  }
  if (ok && t) {
    os_of(t)->notification = id;
  }
  else if (ok) {
    S.own_notification = id;
  }
  else if (!t) {
    own_icon_end();
  }
  return ok;
}

bool bk_shell_os_progress(BKWindow *win, int state, float value)
{
  HWND hwnd = (HWND)BK_window_handle(win);
  if (!hwnd) {
    return false;
  }
  if (!S.bar && !S.bar_tried) {
    S.bar_tried = true;
    if (com() && SUCCEEDED(S.co_create(&k_clsid_taskbar, NULL, CLSCTX_INPROC_SERVER,
                                       &k_iid_taskbar3, (void **)&S.bar)) &&
        FAILED(ITaskbarList3_HrInit(S.bar)))
    {
      ITaskbarList3_Release(S.bar);
      S.bar = NULL;
    }
  }
  if (!S.bar) {
    return false;
  }
  static const TBPFLAG flags[] = {TBPF_NOPROGRESS, TBPF_NORMAL, TBPF_INDETERMINATE, TBPF_PAUSED,
                                  TBPF_ERROR};
  /* SetProgressValue sets TBPF_NORMAL itself (GHOST's comment): the state goes after it */
  if (state != BK_PROGRESS_NONE && state != BK_PROGRESS_INDETERMINATE &&
      FAILED(ITaskbarList3_SetProgressValue(S.bar, hwnd, (ULONGLONG)(value * 10000.0f), 10000)))
  {
    return false;
  }
  return SUCCEEDED(ITaskbarList3_SetProgressState(S.bar, hwnd, flags[state]));
}

bool bk_shell_os_add_recent(const char *path)
{
  wchar_t *w = bk_utf8_to_wide(path);
  wchar_t full[MAX_PATH * 4];
  const DWORD n = w ? GetFullPathNameW(w, MAX_PATH * 4, full, NULL) : 0;
  free(w);
  if (n == 0 || n >= MAX_PATH * 4 || GetFileAttributesW(full) == INVALID_FILE_ATTRIBUTES) {
    return false;
  }
  /* With an app id the entry goes to that app's jump list (GHOST's SHARD_APPIDINFO). */
  const char *app = bk_shell_app_id();
  IShellItem *item = NULL;
  if (app && S.item_from_path && com() &&
      SUCCEEDED(S.item_from_path(full, NULL, &k_iid_shell_item, (void **)&item)))
  {
    wchar_t *app_w = bk_utf8_to_wide(app);
    SHARDAPPIDINFO info = {item, app_w};
    S.add_recent(SHARD_APPIDINFO, &info);
    IShellItem_Release(item);
    free(app_w);
    return true;
  }
  S.add_recent(SHARD_PATHW, full);
  return true;
}

#endif /* _WIN32 */
