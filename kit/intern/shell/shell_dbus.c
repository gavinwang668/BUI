/* The desktop shell on Linux and the BSDs (shell.h), over the session bus with libdbus-1, which is
 * loaded at run time (dlopen, as the window hosts load libX11 and libwayland): without it, or
 * without a session bus, the parts that need it are missing and nothing fails.
 *
 *  tray icons      StatusNotifierItem (KDE, GNOME's AppIndicator extension, waybar, ...) with its
 *                  menu as com.canonical.dbusmenu, as tauri's tray-icon (libappindicator) does;
 *                  icons as ARGB pixmaps, so no icon theme is needed
 *  notifications   org.freedesktop.Notifications (notify-rust): its "default" action is the click
 *  progress        com.canonical.Unity.LauncherEntry (KDE's task manager, Dash to Dock, Plank),
 *                  per app: the desktop file of BK_shell_set_app_id
 *  recent files    $XDG_DATA_HOME/recently-used.xbel, GTK's list (no D-Bus); Blender's GHOST
 *                  has none here (GHOST_SystemPathsUnix::addToSystemRecentFiles is a TODO)
 *
 * Single-threaded: everything runs on the UI thread, whose window loop waits on bk_shell_fd()
 * and calls bk_shell_poll(), which reads and dispatches what arrived.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#if defined(__linux__) || defined(__FreeBSD__) || defined(__NetBSD__) || defined(__OpenBSD__) || \
    defined(__DragonFly__)

#  include <dlfcn.h>
#  include <errno.h>
#  include <limits.h>
#  include <stdint.h>
#  include <stdio.h>
#  include <stdlib.h>
#  include <string.h>
#  include <sys/stat.h>
#  include <time.h>
#  include <unistd.h>

#  include "kit_intern.h"
#  include "shell.h"

/* -- libdbus-1's ABI (dbus/dbus.h), declared here: building needs no headers -- */
typedef struct DBusConnection DBusConnection;
typedef struct DBusMessage DBusMessage;
typedef struct DBusPendingCall DBusPendingCall;
typedef uint32_t dbus_bool_t;
typedef struct DBusError {
  const char *name, *message;
  unsigned int dummy : 5;
  void *padding;
} DBusError;
typedef struct DBusMessageIter { /* opaque, at least the library's size */
  void *opaque[16];
} DBusMessageIter;
typedef int (*DBusFilterFn)(DBusConnection *, DBusMessage *, void *);
typedef void (*DBusPendingFn)(DBusPendingCall *, void *);
enum { BUS_SESSION = 0 };
enum { MSG_METHOD_CALL = 1, MSG_SIGNAL = 4 };
enum { HANDLED = 0, NOT_YET_HANDLED = 1 };
enum { DATA_REMAINS = 0 };
enum { NAME_DO_NOT_QUEUE = 4, NAME_PRIMARY_OWNER = 1 };
enum {
  T_BYTE = 'y',
  T_BOOL = 'b',
  T_INT = 'i',
  T_UINT = 'u',
  T_DOUBLE = 'd',
  T_STRING = 's',
  T_PATH = 'o',
  T_ARRAY = 'a',
  T_VARIANT = 'v',
  T_STRUCT = 'r',
  T_ENTRY = 'e',
};

#  define DBUS_FUNCS(X) \
    X(void, dbus_error_init, (DBusError *)) \
    X(void, dbus_error_free, (DBusError *)) \
    X(DBusConnection *, dbus_bus_get_private, (int, DBusError *)) \
    X(void, dbus_connection_set_exit_on_disconnect, (DBusConnection *, dbus_bool_t)) \
    X(void, dbus_connection_close, (DBusConnection *)) \
    X(void, dbus_connection_unref, (DBusConnection *)) \
    X(int, dbus_bus_request_name, (DBusConnection *, const char *, unsigned, DBusError *)) \
    X(int, dbus_bus_release_name, (DBusConnection *, const char *, DBusError *)) \
    X(void, dbus_bus_add_match, (DBusConnection *, const char *, DBusError *)) \
    X(dbus_bool_t, dbus_bus_name_has_owner, (DBusConnection *, const char *, DBusError *)) \
    X(dbus_bool_t, dbus_connection_add_filter, \
      (DBusConnection *, DBusFilterFn, void *, void (*)(void *))) \
    X(dbus_bool_t, dbus_connection_get_unix_fd, (DBusConnection *, int *)) \
    X(dbus_bool_t, dbus_connection_read_write, (DBusConnection *, int)) \
    X(int, dbus_connection_dispatch, (DBusConnection *)) \
    X(dbus_bool_t, dbus_connection_send, (DBusConnection *, DBusMessage *, uint32_t *)) \
    X(dbus_bool_t, dbus_connection_send_with_reply, \
      (DBusConnection *, DBusMessage *, DBusPendingCall **, int)) \
    X(void, dbus_connection_flush, (DBusConnection *)) \
    X(dbus_bool_t, dbus_pending_call_set_notify, \
      (DBusPendingCall *, DBusPendingFn, void *, void (*)(void *))) \
    X(DBusMessage *, dbus_pending_call_steal_reply, (DBusPendingCall *)) \
    X(void, dbus_pending_call_unref, (DBusPendingCall *)) \
    X(DBusMessage *, dbus_message_new_method_call, \
      (const char *, const char *, const char *, const char *)) \
    X(DBusMessage *, dbus_message_new_method_return, (DBusMessage *)) \
    X(DBusMessage *, dbus_message_new_signal, (const char *, const char *, const char *)) \
    X(DBusMessage *, dbus_message_new_error, (DBusMessage *, const char *, const char *)) \
    X(void, dbus_message_unref, (DBusMessage *)) \
    X(int, dbus_message_get_type, (DBusMessage *)) \
    X(const char *, dbus_message_get_interface, (DBusMessage *)) \
    X(const char *, dbus_message_get_member, (DBusMessage *)) \
    X(const char *, dbus_message_get_path, (DBusMessage *)) \
    X(const char *, dbus_message_get_destination, (DBusMessage *)) \
    X(dbus_bool_t, dbus_message_iter_init, (DBusMessage *, DBusMessageIter *)) \
    X(void, dbus_message_iter_init_append, (DBusMessage *, DBusMessageIter *)) \
    X(dbus_bool_t, dbus_message_iter_append_basic, (DBusMessageIter *, int, const void *)) \
    X(dbus_bool_t, dbus_message_iter_append_fixed_array, \
      (DBusMessageIter *, int, const void *, int)) \
    X(dbus_bool_t, dbus_message_iter_open_container, \
      (DBusMessageIter *, int, const char *, DBusMessageIter *)) \
    X(dbus_bool_t, dbus_message_iter_close_container, (DBusMessageIter *, DBusMessageIter *)) \
    X(int, dbus_message_iter_get_arg_type, (DBusMessageIter *)) \
    X(void, dbus_message_iter_get_basic, (DBusMessageIter *, void *)) \
    X(dbus_bool_t, dbus_message_iter_next, (DBusMessageIter *)) \
    X(void, dbus_message_iter_recurse, (DBusMessageIter *, DBusMessageIter *))

#  define DBUS_FIELD(ret, name, params) ret(*name) params;
static struct {
  DBUS_FUNCS(DBUS_FIELD)
} D;
#  undef DBUS_FIELD

#  define SNI_PATH "/StatusNotifierItem"
#  define SNI_IFACE "org.kde.StatusNotifierItem"
#  define WATCHER "org.kde.StatusNotifierWatcher"
#  define MENU_IFACE "com.canonical.dbusmenu"
#  define MENU_PATH "/MenuBar/"
#  define PROPS_IFACE "org.freedesktop.DBus.Properties"
#  define NOTIFY_NAME "org.freedesktop.Notifications"
#  define NOTIFY_PATH "/org/freedesktop/Notifications"
#  define ICON_SIZE 32

static DBusConnection *g_conn;
static bool g_watcher; /* a StatusNotifierWatcher is on the bus */

/* One tray's D-Bus side: its bus name, its icon as SNI pixmap bytes, its menu's revision. */
typedef struct TrayDBus {
  char name[96];
  unsigned char *argb; /* ICON_SIZE^2 big-endian ARGB, straight alpha; NULL: none */
  uint32_t revision;
} TrayDBus;

/* Notifications on screen: ours (BK_notify's id) for the server's. */
#  define MAX_NOTES 32
static struct {
  int id;
  uint32_t server;
} g_notes[MAX_NOTES];

static TrayDBus *os_of(const BKTray *t)
{
  return (TrayDBus *)t->os;
}

/* -- the connection -- */

static bool load_dbus(void)
{
  void *lib = dlopen("libdbus-1.so.3", RTLD_NOW | RTLD_LOCAL);
  if (!lib) {
    return false;
  }
#  define DBUS_LOAD(ret, name, params) \
    if (!(*(void **)&D.name = dlsym(lib, #name))) { \
      dlclose(lib); \
      return false; \
    }
  DBUS_FUNCS(DBUS_LOAD)
#  undef DBUS_LOAD
  return true;
}

static void drop_connection(void)
{
  if (g_conn) {
    D.dbus_connection_close(g_conn);
    D.dbus_connection_unref(g_conn);
    g_conn = NULL;
  }
}

static int filter(DBusConnection *c, DBusMessage *m, void *data);

/* Whether the session has a bus to connect to: without one, libdbus would "autolaunch" a bus
 * daemon of its own (dbus-launch), which outlives the program and serves no desktop. */
static bool session_bus_exists(void)
{
  const char *addr = getenv("DBUS_SESSION_BUS_ADDRESS"), *run = getenv("XDG_RUNTIME_DIR");
  char path[PATH_MAX];
  if (addr && addr[0]) {
    return true;
  }
  return run && run[0] && snprintf(path, sizeof(path), "%s/bus", run) < (int)sizeof(path) &&
         access(path, F_OK) == 0;
}

static bool connect_bus(void)
{
  if (!session_bus_exists() || !load_dbus()) {
    return false;
  }
  DBusError err;
  D.dbus_error_init(&err);
  g_conn = D.dbus_bus_get_private(BUS_SESSION, &err);
  D.dbus_error_free(&err);
  if (!g_conn) {
    return false;
  }
  D.dbus_connection_set_exit_on_disconnect(g_conn, 0); /* the bus going away is not ours to end */
  D.dbus_connection_add_filter(g_conn, filter, NULL, NULL);
  D.dbus_bus_add_match(g_conn, "type='signal',interface='" NOTIFY_NAME "'", NULL);
  D.dbus_bus_add_match(g_conn,
                       "type='signal',sender='org.freedesktop.DBus',member='NameOwnerChanged',"
                       "arg0='" WATCHER "'",
                       NULL);
  g_watcher = D.dbus_bus_name_has_owner(g_conn, WATCHER, NULL) != 0;
  return true;
}

/* While a message is dispatched (a listener may run a loop of its own: a modal dialog), the
 * connection is not read again: libdbus dispatches one message at a time. */
static bool g_dispatching;

int bk_shell_fd(void)
{
  int fd = -1;
  return (g_conn && !g_dispatching && D.dbus_connection_get_unix_fd(g_conn, &fd)) ? fd : -1;
}

void bk_shell_poll(void)
{
  if (!g_conn || g_dispatching) {
    return;
  }
  if (!D.dbus_connection_read_write(g_conn, 0)) { /* disconnected: stop waiting on it */
    drop_connection();
    return;
  }
  g_dispatching = true;
  while (g_conn && D.dbus_connection_dispatch(g_conn) == DATA_REMAINS) {
  }
  g_dispatching = false;
}

static void send_msg(DBusMessage *m)
{
  if (m) {
    D.dbus_connection_send(g_conn, m, NULL);
    D.dbus_message_unref(m);
    D.dbus_connection_flush(g_conn);
  }
}

/* -- writing values -- */

static void put(DBusMessageIter *it, int type, const void *v)
{
  D.dbus_message_iter_append_basic(it, type, v);
}

static void put_str(DBusMessageIter *it, const char *s)
{
  put(it, T_STRING, &s);
}

/* A variant holding one basic value of `type` (signature `sig`). */
static void put_var(DBusMessageIter *it, const char *sig, int type, const void *v)
{
  DBusMessageIter var;
  D.dbus_message_iter_open_container(it, T_VARIANT, sig, &var);
  put(&var, type, v);
  D.dbus_message_iter_close_container(it, &var);
}

static void put_var_str(DBusMessageIter *it, const char *s)
{
  put_var(it, "s", T_STRING, &s);
}

static void put_var_bool(DBusMessageIter *it, bool b)
{
  const dbus_bool_t v = b;
  put_var(it, "b", T_BOOL, &v);
}

static void put_var_int(DBusMessageIter *it, int32_t i)
{
  put_var(it, "i", T_INT, &i);
}

/* An a{sv} entry: the key, and the value written by the caller into `*r_value` (a variant's
 * parent), closed with entry_end. */
static void entry_begin(DBusMessageIter *dict, DBusMessageIter *entry, const char *key)
{
  D.dbus_message_iter_open_container(dict, T_ENTRY, NULL, entry);
  put_str(entry, key);
}

static void entry_end(DBusMessageIter *dict, DBusMessageIter *entry)
{
  D.dbus_message_iter_close_container(dict, entry);
}

/* An icon as (iiay) pixmaps' bytes: big-endian ARGB, straight alpha (the SNI spec). */
static unsigned char *icon_argb(const char *icon)
{
  const float white[4] = {1.0f, 1.0f, 1.0f, 1.0f}; /* panels are dark more often than not */
  BKImBuf *ib = bk_shell_icon(icon, ICON_SIZE, white);
  unsigned char *out = ib ? (unsigned char *)malloc(ICON_SIZE * ICON_SIZE * 4) : NULL;
  if (out && BK_imbuf_rgba(ib, 0, out)) {
    for (int i = 0; i < ICON_SIZE * ICON_SIZE; i++) {
      unsigned char *p = &out[i * 4];
      const unsigned char r = p[0], g = p[1], b = p[2], a = p[3];
      p[0] = a;
      p[1] = r;
      p[2] = g;
      p[3] = b;
    }
  }
  else {
    free(out);
    out = NULL;
  }
  BK_imbuf_free(ib);
  return out;
}

/* a(iiay): no pixmap when `argb` is NULL */
static void put_pixmaps(DBusMessageIter *it, const unsigned char *argb)
{
  DBusMessageIter arr, st, bytes;
  D.dbus_message_iter_open_container(it, T_ARRAY, "(iiay)", &arr);
  if (argb) {
    const int32_t size = ICON_SIZE;
    D.dbus_message_iter_open_container(&arr, T_STRUCT, NULL, &st);
    put(&st, T_INT, &size);
    put(&st, T_INT, &size);
    D.dbus_message_iter_open_container(&st, T_ARRAY, "y", &bytes);
    D.dbus_message_iter_append_fixed_array(&bytes, T_BYTE, &argb, ICON_SIZE * ICON_SIZE * 4);
    D.dbus_message_iter_close_container(&st, &bytes);
    D.dbus_message_iter_close_container(&arr, &st);
  }
  D.dbus_message_iter_close_container(it, &arr);
}

/* -- the app's name -- */

static const char *app_id(void)
{
  static char exe[256];
  const char *id = bk_shell_app_id();
  if (id) {
    return id;
  }
  if (!exe[0]) {
    char path[PATH_MAX];
    const ssize_t n = readlink("/proc/self/exe", path, sizeof(path) - 1);
    const char *base = "bui";
    if (n > 0) {
      path[n] = 0;
      const char *slash = strrchr(path, '/');
      base = slash ? slash + 1 : path;
    }
    snprintf(exe, sizeof(exe), "%.255s", base);
  }
  return exe;
}

/* -- StatusNotifierItem -- */

static BKTray *tray_by_uid(int uid)
{
  for (BKTray *t = bk_shell_trays; t; t = t->next) {
    if (t->uid == uid) {
      return t;
    }
  }
  return NULL;
}

/* The tray a call is for: by the bus name it was sent to, else (sent to our unique name) the
 * only one there is. */
static BKTray *tray_of_call(DBusMessage *m)
{
  const char *dest = D.dbus_message_get_destination(m);
  for (BKTray *t = bk_shell_trays; t && dest; t = t->next) {
    if (strcmp(dest, os_of(t)->name) == 0) {
      return t;
    }
  }
  return (bk_shell_trays && !bk_shell_trays->next) ? bk_shell_trays : NULL;
}

static void sni_prop(DBusMessageIter *it, const BKTray *t, const char *name)
{
  char menu[48];
  if (strcmp(name, "Category") == 0) {
    put_var_str(it, "ApplicationStatus");
  }
  else if (strcmp(name, "Id") == 0) {
    put_var_str(it, app_id());
  }
  else if (strcmp(name, "Title") == 0) {
    put_var_str(it, t->tooltip[0] ? t->tooltip : app_id());
  }
  else if (strcmp(name, "Status") == 0) {
    put_var_str(it, "Active");
  }
  else if (strcmp(name, "IconName") == 0 || strcmp(name, "IconThemePath") == 0) {
    put_var_str(it, "");
  }
  else if (strcmp(name, "IconPixmap") == 0) {
    DBusMessageIter var;
    D.dbus_message_iter_open_container(it, T_VARIANT, "a(iiay)", &var);
    put_pixmaps(&var, os_of(t)->argb);
    D.dbus_message_iter_close_container(it, &var);
  }
  else if (strcmp(name, "ItemIsMenu") == 0) {
    put_var_bool(it, false);
  }
  else if (strcmp(name, "WindowId") == 0) {
    put_var_int(it, 0);
  }
  else if (strcmp(name, "Menu") == 0) {
    snprintf(menu, sizeof(menu), MENU_PATH "%d", t->uid);
    const char *path = menu;
    put_var(it, "o", T_PATH, &path);
  }
  else if (strcmp(name, "ToolTip") == 0) { /* (sa(iiay)ss): icon, pixmaps, title, text */
    DBusMessageIter var, st;
    D.dbus_message_iter_open_container(it, T_VARIANT, "(sa(iiay)ss)", &var);
    D.dbus_message_iter_open_container(&var, T_STRUCT, NULL, &st);
    put_str(&st, "");
    put_pixmaps(&st, NULL);
    put_str(&st, t->tooltip);
    put_str(&st, "");
    D.dbus_message_iter_close_container(&var, &st);
    D.dbus_message_iter_close_container(it, &var);
  }
}

static const char *const k_sni_props[] = {"Category", "Id", "Title", "Status", "IconName",
                                          "IconThemePath", "IconPixmap", "ItemIsMenu",
                                          "WindowId", "Menu", "ToolTip"};

static bool is_sni_prop(const char *name)
{
  for (size_t i = 0; i < sizeof(k_sni_props) / sizeof(k_sni_props[0]); i++) {
    if (strcmp(name, k_sni_props[i]) == 0) {
      return true;
    }
  }
  return false;
}

static void signal_tray(const BKTray *t, const char *member)
{
  (void)t; /* every tray's object is at SNI_PATH: the bus name tells them apart */
  send_msg(D.dbus_message_new_signal(SNI_PATH, SNI_IFACE, member));
}

static void register_tray(const BKTray *t)
{
  DBusMessage *m = D.dbus_message_new_method_call(WATCHER, "/StatusNotifierWatcher", WATCHER,
                                                  "RegisterStatusNotifierItem");
  if (m) {
    DBusMessageIter it;
    D.dbus_message_iter_init_append(m, &it);
    put_str(&it, os_of(t)->name);
  }
  send_msg(m);
}

/* -- com.canonical.dbusmenu -- */

/* How many items a menu has, its submenus' too. */
static int menu_count(const BKMenuSpec *items, int n)
{
  int c = n;
  for (int i = 0; i < n; i++) {
    c += menu_count(items[i].sub, items[i].nsub);
  }
  return c;
}

/* dbusmenu labels take '_' as the mnemonic mark: a '_' of the text is doubled. */
static void put_label(DBusMessageIter *it, const char *text)
{
  size_t n = 0;
  for (const char *s = text; *s; s++) {
    n += *s == '_' ? 2 : 1;
  }
  char stack[256];
  char *buf = n < sizeof(stack) ? stack : (char *)malloc(n + 1);
  if (!buf) {
    put_var_str(it, "");
    return;
  }
  char *o = buf;
  for (const char *s = text; *s; s++) {
    *o++ = *s;
    if (*s == '_') {
      *o++ = '_';
    }
  }
  *o = 0;
  put_var_str(it, buf);
  if (buf != stack) {
    free(buf);
  }
}

/* Whether property `name` of an item (NULL: the root) is wanted: `names` empty is all. */
static bool wanted(DBusMessageIter *names, const char *name)
{
  if (!names) {
    return true;
  }
  DBusMessageIter it = *names;
  bool any = false;
  while (D.dbus_message_iter_get_arg_type(&it) == T_STRING) {
    const char *s;
    D.dbus_message_iter_get_basic(&it, &s);
    if (strcmp(s, name) == 0) {
      return true;
    }
    any = true;
    D.dbus_message_iter_next(&it);
  }
  return !any;
}

/* An item's properties, a{sv}; `item` NULL: the root. */
static void put_item_props(DBusMessageIter *it, const BKMenuSpec *item, DBusMessageIter *names)
{
  DBusMessageIter dict, e;
  D.dbus_message_iter_open_container(it, T_ARRAY, "{sv}", &dict);
#  define PROP(key) (wanted(names, key) && (entry_begin(&dict, &e, key), true))
  if (!item || item->nsub > 0) {
    if (PROP("children-display")) {
      put_var_str(&e, "submenu");
      entry_end(&dict, &e);
    }
  }
  if (item && bk_menuspec_is_sep(item)) {
    if (PROP("type")) {
      put_var_str(&e, "separator");
      entry_end(&dict, &e);
    }
  }
  else if (item) {
    if (PROP("label")) {
      put_label(&e, item->text);
      entry_end(&dict, &e);
    }
    if (PROP("enabled")) {
      put_var_bool(&e, item->enabled != 0);
      entry_end(&dict, &e);
    }
    if (item->checked >= 0) {
      if (PROP("toggle-type")) {
        put_var_str(&e, "checkmark");
        entry_end(&dict, &e);
      }
      if (PROP("toggle-state")) {
        put_var_int(&e, item->checked > 0);
        entry_end(&dict, &e);
      }
    }
  }
#  undef PROP
  D.dbus_message_iter_close_container(it, &dict);
}

/* (ia{sv}av): item `id` and, `depth` levels down (-1: all), its children, which are numbered
 * from id + 1 (shell.h's pre-order). */
static void put_layout(DBusMessageIter *it, int id, const BKMenuSpec *item, const BKMenuSpec *kids,
                       int nkids, int depth, DBusMessageIter *names)
{
  DBusMessageIter st, arr, var;
  D.dbus_message_iter_open_container(it, T_STRUCT, NULL, &st);
  const int32_t id32 = id;
  put(&st, T_INT, &id32);
  put_item_props(&st, item, names);
  D.dbus_message_iter_open_container(&st, T_ARRAY, "v", &arr);
  int child = id + 1;
  for (int i = 0; depth != 0 && i < nkids; i++) {
    D.dbus_message_iter_open_container(&arr, T_VARIANT, "(ia{sv}av)", &var);
    put_layout(&var, child, &kids[i], kids[i].sub, kids[i].nsub, depth - 1, names);
    D.dbus_message_iter_close_container(&arr, &var);
    child += 1 + menu_count(kids[i].sub, kids[i].nsub);
  }
  D.dbus_message_iter_close_container(&st, &arr);
  D.dbus_message_iter_close_container(it, &st);
}

/* The menu of the tray a dbusmenu path names ("/MenuBar/<uid>"). */
static BKTray *tray_of_menu(const char *path)
{
  if (!path || strncmp(path, MENU_PATH, sizeof(MENU_PATH) - 1) != 0) {
    return NULL;
  }
  return tray_by_uid(atoi(path + sizeof(MENU_PATH) - 1));
}

static void menu_clicked(BKTray *t, int id, const char *event)
{
  if (strcmp(event, "clicked") == 0) {
    bk_shell_menu_chosen(t, id);
  }
}

/* A method call to a menu; the reply is built into `r`. Returns false for an unknown method. */
static bool menu_call(BKTray *t, DBusMessage *m, const char *member, DBusMessage *r)
{
  DBusMessageIter in, out;
  const bool has_args = D.dbus_message_iter_init(m, &in) != 0;
  D.dbus_message_iter_init_append(r, &out);
  if (strcmp(member, "GetLayout") == 0) { /* (i parent, i depth, as names) -> (u, layout) */
    int32_t parent = 0, depth = -1;
    DBusMessageIter names;
    bool have_names = false;
    if (has_args && D.dbus_message_iter_get_arg_type(&in) == T_INT) {
      D.dbus_message_iter_get_basic(&in, &parent);
      D.dbus_message_iter_next(&in);
      if (D.dbus_message_iter_get_arg_type(&in) == T_INT) {
        D.dbus_message_iter_get_basic(&in, &depth);
        D.dbus_message_iter_next(&in);
      }
      if (D.dbus_message_iter_get_arg_type(&in) == T_ARRAY) {
        D.dbus_message_iter_recurse(&in, &names);
        have_names = true;
      }
    }
    put(&out, T_UINT, &os_of(t)->revision);
    const BKMenuSpec *item = bk_shell_menu_item(t->menu, t->nmenu, parent);
    if (parent != 0 && !item) {
      parent = 0;
    }
    put_layout(&out, parent, item, item ? item->sub : t->menu, item ? item->nsub : t->nmenu,
               depth, have_names ? &names : NULL);
    return true;
  }
  if (strcmp(member, "GetGroupProperties") == 0) { /* (ai ids, as names) -> a(ia{sv}) */
    DBusMessageIter ids, names, arr, st;
    bool have_names = false;
    if (!has_args || D.dbus_message_iter_get_arg_type(&in) != T_ARRAY) {
      return false;
    }
    D.dbus_message_iter_recurse(&in, &ids);
    D.dbus_message_iter_next(&in);
    if (D.dbus_message_iter_get_arg_type(&in) == T_ARRAY) {
      D.dbus_message_iter_recurse(&in, &names);
      have_names = true;
    }
    D.dbus_message_iter_open_container(&out, T_ARRAY, "(ia{sv})", &arr);
    while (D.dbus_message_iter_get_arg_type(&ids) == T_INT) {
      int32_t id;
      D.dbus_message_iter_get_basic(&ids, &id);
      const BKMenuSpec *item = bk_shell_menu_item(t->menu, t->nmenu, id);
      if (item || id == 0) {
        D.dbus_message_iter_open_container(&arr, T_STRUCT, NULL, &st);
        put(&st, T_INT, &id);
        put_item_props(&st, item, have_names ? &names : NULL);
        D.dbus_message_iter_close_container(&arr, &st);
      }
      D.dbus_message_iter_next(&ids);
    }
    D.dbus_message_iter_close_container(&out, &arr);
    return true;
  }
  if (strcmp(member, "Event") == 0) { /* (i id, s event, v data, u time) */
    int32_t id = 0;
    const char *event = "";
    if (has_args && D.dbus_message_iter_get_arg_type(&in) == T_INT) {
      D.dbus_message_iter_get_basic(&in, &id);
      D.dbus_message_iter_next(&in);
      if (D.dbus_message_iter_get_arg_type(&in) == T_STRING) {
        D.dbus_message_iter_get_basic(&in, &event);
      }
    }
    menu_clicked(t, id, event);
    return true;
  }
  if (strcmp(member, "EventGroup") == 0) { /* a(isvu) -> ai (ids not found) */
    DBusMessageIter arr, st, errs;
    const int uid = t->uid;
    if (has_args && D.dbus_message_iter_get_arg_type(&in) == T_ARRAY) {
      D.dbus_message_iter_recurse(&in, &arr);
      while (D.dbus_message_iter_get_arg_type(&arr) == T_STRUCT && tray_by_uid(uid) == t) {
        int32_t id = 0;
        const char *event = "";
        D.dbus_message_iter_recurse(&arr, &st);
        D.dbus_message_iter_get_basic(&st, &id);
        D.dbus_message_iter_next(&st);
        D.dbus_message_iter_get_basic(&st, &event);
        menu_clicked(t, id, event);
        D.dbus_message_iter_next(&arr);
      }
    }
    D.dbus_message_iter_open_container(&out, T_ARRAY, "i", &errs);
    D.dbus_message_iter_close_container(&out, &errs);
    return true;
  }
  if (strcmp(member, "AboutToShow") == 0) { /* -> b needUpdate */
    const dbus_bool_t no = 0;
    put(&out, T_BOOL, &no);
    return true;
  }
  if (strcmp(member, "AboutToShowGroup") == 0) { /* -> (ai updatesNeeded, ai idErrors) */
    DBusMessageIter a;
    for (int i = 0; i < 2; i++) {
      D.dbus_message_iter_open_container(&out, T_ARRAY, "i", &a);
      D.dbus_message_iter_close_container(&out, &a);
    }
    return true;
  }
  return false;
}

static void menu_prop(DBusMessageIter *it, const char *name)
{
  if (strcmp(name, "Version") == 0) {
    const uint32_t v = 3;
    put_var(it, "u", T_UINT, &v);
  }
  else if (strcmp(name, "TextDirection") == 0) {
    put_var_str(it, "ltr");
  }
  else if (strcmp(name, "Status") == 0) {
    put_var_str(it, "normal");
  }
  else { /* IconThemePath: as */
    DBusMessageIter var, arr;
    D.dbus_message_iter_open_container(it, T_VARIANT, "as", &var);
    D.dbus_message_iter_open_container(&var, T_ARRAY, "s", &arr);
    D.dbus_message_iter_close_container(&var, &arr);
    D.dbus_message_iter_close_container(it, &var);
  }
}

static const char *const k_menu_props[] = {"Version", "TextDirection", "Status", "IconThemePath"};

/* -- org.freedesktop.DBus.Properties for both objects -- */

/* Get (s iface, s name) -> v; GetAll (s iface) -> a{sv}. */
static bool props_call(BKTray *t, bool menu, DBusMessage *m, const char *member, DBusMessage *r)
{
  DBusMessageIter in, out;
  const char *iface = "", *name = "";
  if (!D.dbus_message_iter_init(m, &in) || D.dbus_message_iter_get_arg_type(&in) != T_STRING) {
    return false;
  }
  D.dbus_message_iter_get_basic(&in, &iface);
  D.dbus_message_iter_next(&in);
  D.dbus_message_iter_init_append(r, &out);
  const char *const *keys = menu ? k_menu_props : k_sni_props;
  const size_t nkeys = menu ? sizeof(k_menu_props) / sizeof(k_menu_props[0]) :
                              sizeof(k_sni_props) / sizeof(k_sni_props[0]);
  if (strcmp(member, "GetAll") == 0) {
    DBusMessageIter dict, e;
    D.dbus_message_iter_open_container(&out, T_ARRAY, "{sv}", &dict);
    for (size_t i = 0; i < nkeys; i++) {
      entry_begin(&dict, &e, keys[i]);
      if (menu) {
        menu_prop(&e, keys[i]);
      }
      else {
        sni_prop(&e, t, keys[i]);
      }
      entry_end(&dict, &e);
    }
    D.dbus_message_iter_close_container(&out, &dict);
    return true;
  }
  if (strcmp(member, "Get") != 0 || D.dbus_message_iter_get_arg_type(&in) != T_STRING) {
    return false;
  }
  D.dbus_message_iter_get_basic(&in, &name);
  if (menu) {
    for (size_t i = 0; i < nkeys; i++) {
      if (strcmp(name, keys[i]) == 0) {
        menu_prop(&out, name);
        return true;
      }
    }
    return false;
  }
  if (!is_sni_prop(name)) {
    return false;
  }
  sni_prop(&out, t, name);
  return true;
}

/* -- notifications -- */

static void notify_reply(DBusPendingCall *pending, void *data)
{
  const int id = (int)(intptr_t)data;
  DBusMessage *r = D.dbus_pending_call_steal_reply(pending);
  DBusMessageIter it;
  if (r && D.dbus_message_iter_init(r, &it) && D.dbus_message_iter_get_arg_type(&it) == T_UINT) {
    uint32_t server;
    D.dbus_message_iter_get_basic(&it, &server);
    static int next = 0;
    g_notes[next].id = id;
    g_notes[next].server = server;
    next = (next + 1) % MAX_NOTES; /* the oldest goes: it has long been answered or gone */
  }
  if (r) {
    D.dbus_message_unref(r);
  }
  D.dbus_pending_call_unref(pending);
}

static void notification_signal(DBusMessage *m, const char *member)
{
  DBusMessageIter it;
  uint32_t server = 0;
  const char *action = "";
  if (!D.dbus_message_iter_init(m, &it) || D.dbus_message_iter_get_arg_type(&it) != T_UINT) {
    return;
  }
  D.dbus_message_iter_get_basic(&it, &server);
  D.dbus_message_iter_next(&it);
  if (D.dbus_message_iter_get_arg_type(&it) == T_STRING) {
    D.dbus_message_iter_get_basic(&it, &action);
  }
  for (int i = 0; i < MAX_NOTES; i++) {
    if (g_notes[i].id && g_notes[i].server == server) {
      const int id = g_notes[i].id;
      if (strcmp(member, "NotificationClosed") == 0) {
        g_notes[i].id = 0;
      }
      else if (strcmp(member, "ActionInvoked") == 0 && strcmp(action, "default") == 0) {
        bk_shell_emit(NULL, BK_EV_ACTIVATE, NULL, id, 0);
      }
      return;
    }
  }
}

/* -- the filter: every message that comes in -- */

static int filter(DBusConnection *c, DBusMessage *m, void *data)
{
  (void)c;
  (void)data;
  const char *iface = D.dbus_message_get_interface(m);
  const char *member = D.dbus_message_get_member(m);
  const char *path = D.dbus_message_get_path(m);
  if (!iface || !member) {
    return NOT_YET_HANDLED;
  }
  if (D.dbus_message_get_type(m) == MSG_SIGNAL) {
    if (strcmp(iface, NOTIFY_NAME) == 0) {
      notification_signal(m, member);
    }
    else if (strcmp(member, "NameOwnerChanged") == 0) { /* the watcher (re)started or left */
      DBusMessageIter it;
      const char *name = "", *owner = "";
      if (D.dbus_message_iter_init(m, &it) && D.dbus_message_iter_get_arg_type(&it) == T_STRING) {
        D.dbus_message_iter_get_basic(&it, &name);
        D.dbus_message_iter_next(&it);
        D.dbus_message_iter_next(&it);
        if (D.dbus_message_iter_get_arg_type(&it) == T_STRING) {
          D.dbus_message_iter_get_basic(&it, &owner);
        }
      }
      if (strcmp(name, WATCHER) == 0) {
        g_watcher = owner[0] != 0;
        for (BKTray *t = bk_shell_trays; t && g_watcher; t = t->next) {
          register_tray(t);
        }
      }
    }
    return NOT_YET_HANDLED;
  }
  if (D.dbus_message_get_type(m) != MSG_METHOD_CALL || !path) {
    return NOT_YET_HANDLED;
  }
  const bool menu = strncmp(path, MENU_PATH, sizeof(MENU_PATH) - 1) == 0;
  BKTray *t = menu ? tray_of_menu(path) : (strcmp(path, SNI_PATH) == 0 ? tray_of_call(m) : NULL);
  if (!t) {
    return NOT_YET_HANDLED;
  }
  DBusMessage *r = D.dbus_message_new_method_return(m);
  if (!r) {
    return HANDLED;
  }
  bool ok = false;
  if (strcmp(iface, PROPS_IFACE) == 0) {
    ok = props_call(t, menu, m, member, r);
  }
  else if (strcmp(iface, "org.freedesktop.DBus.Introspectable") == 0 &&
           strcmp(member, "Introspect") == 0)
  { /* the interfaces, for tools and the panels that ask (the methods are the specs') */
    DBusMessageIter out;
    D.dbus_message_iter_init_append(r, &out);
    put_str(&out, menu ? "<node><interface name=\"" MENU_IFACE "\"/><interface name=\"" PROPS_IFACE
                         "\"/></node>" :
                         "<node><interface name=\"" SNI_IFACE "\"/><interface name=\"" PROPS_IFACE
                         "\"/></node>");
    ok = true;
  }
  else if (menu && strcmp(iface, MENU_IFACE) == 0) {
    ok = menu_call(t, m, member, r);
  }
  else if (!menu && strcmp(iface, SNI_IFACE) == 0) {
    /* Activate / SecondaryActivate / ContextMenu (i x, i y), Scroll (i, s) */
    ok = true;
    if (strcmp(member, "Activate") == 0) {
      bk_shell_emit(t, BK_EV_CLICK, NULL, 1, 0);
    }
    else if (strcmp(member, "SecondaryActivate") == 0) {
      bk_shell_emit(t, BK_EV_CLICK, NULL, 2, 0);
    }
    else if (strcmp(member, "ContextMenu") == 0) {
      if (t->nmenu == 0) { /* with a menu, the host shows it */
        bk_shell_emit(t, BK_EV_CLICK, NULL, 3, 0);
      }
    }
    else {
      ok = strcmp(member, "Scroll") == 0 || strcmp(member, "ProvideXdgActivationToken") == 0;
    }
  }
  if (!ok) {
    D.dbus_message_unref(r);
    r = D.dbus_message_new_error(m, "org.freedesktop.DBus.Error.UnknownMethod", member);
  }
  send_msg(r);
  return HANDLED;
}

/* -- recent files: GTK's recently-used.xbel -- */

static void xml_escape(FILE *f, const char *s)
{
  for (; *s; s++) {
    switch (*s) {
      case '&':
        fputs("&amp;", f);
        break;
      case '<':
        fputs("&lt;", f);
        break;
      case '>':
        fputs("&gt;", f);
        break;
      case '"':
        fputs("&quot;", f);
        break;
      case '\'':
        fputs("&apos;", f);
        break;
      default:
        fputc(*s, f);
    }
  }
}

/* "file://" and the path, percent-encoded (RFC 3986's unreserved characters and '/' stay):
 * nothing in it needs XML escaping. */
static char *file_uri(const char *path)
{
  char *uri = (char *)malloc(strlen(path) * 3 + 8);
  if (!uri) {
    return NULL;
  }
  char *o = uri + sprintf(uri, "file://");
  for (const unsigned char *s = (const unsigned char *)path; *s; s++) {
    if ((*s >= 'a' && *s <= 'z') || (*s >= 'A' && *s <= 'Z') || (*s >= '0' && *s <= '9') ||
        strchr("-._~/", *s))
    {
      *o++ = (char)*s;
    }
    else {
      o += sprintf(o, "%%%02X", *s);
    }
  }
  *o = 0;
  return uri;
}

static char *read_file(const char *path)
{
  FILE *f = fopen(path, "rb");
  if (!f) {
    return NULL;
  }
  fseek(f, 0, SEEK_END);
  const long n = ftell(f);
  fseek(f, 0, SEEK_SET);
  char *s = n >= 0 ? (char *)malloc((size_t)n + 1) : NULL;
  if (s) {
    s[fread(s, 1, (size_t)n, f)] = 0;
  }
  fclose(f);
  return s;
}

static bool add_recent_xbel(const char *path)
{
  char full[PATH_MAX], file[PATH_MAX], tmp[PATH_MAX + 8];
  if (!realpath(path, full)) {
    return false;
  }
  const char *data = getenv("XDG_DATA_HOME"), *home = getenv("HOME");
  if (data && data[0] == '/') {
    snprintf(file, sizeof(file), "%s", data);
  }
  else if (home && home[0]) {
    snprintf(file, sizeof(file), "%s/.local", home);
    mkdir(file, 0700);
    snprintf(file, sizeof(file), "%s/.local/share", home);
  }
  else {
    return false;
  }
  mkdir(file, 0700);
  strncat(file, "/recently-used.xbel", sizeof(file) - strlen(file) - 1);
  snprintf(tmp, sizeof(tmp), "%s.bui", file);

  char *uri = file_uri(full);
  char *old = read_file(file);
  char *end = old ? strstr(old, "</xbel>") : NULL;
  FILE *f = uri ? fopen(tmp, "wb") : NULL;
  if (!f) {
    free(uri);
    free(old);
    return false;
  }
  if (end) { /* the list as it was, without this file's old entry */
    char key[PATH_MAX * 3 + 32];
    snprintf(key, sizeof(key), "<bookmark href=\"%s\"", uri);
    char *at = strstr(old, key);
    char *after = at ? strstr(at, "</bookmark>") : NULL;
    if (after && after < end) {
      after += strlen("</bookmark>");
      while (*after == '\n' || *after == ' ') {
        after++;
      }
      while (at > old && at[-1] == ' ') {
        at--;
      }
      fwrite(old, 1, (size_t)(at - old), f);
      fwrite(after, 1, (size_t)(end - after), f);
    }
    else {
      fwrite(old, 1, (size_t)(end - old), f);
    }
  }
  else {
    fputs("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
          "<xbel version=\"1.0\"\n"
          "      xmlns:bookmark=\"http://www.freedesktop.org/standards/desktop-bookmarks\"\n"
          "      xmlns:mime=\"http://www.freedesktop.org/standards/shared-mime-info\"\n>\n",
          f);
  }
  char stamp[32];
  const time_t now = time(NULL);
  struct tm tm;
  gmtime_r(&now, &tm);
  strftime(stamp, sizeof(stamp), "%Y-%m-%dT%H:%M:%S.000000Z", &tm);
  fprintf(f,
          "  <bookmark href=\"%s\" added=\"%s\" modified=\"%s\" visited=\"%s\">\n"
          "    <info>\n"
          "      <metadata owner=\"http://freedesktop.org\">\n"
          "        <mime:mime-type type=\"application/octet-stream\"/>\n"
          "        <bookmark:applications>\n"
          "          <bookmark:application name=\"",
          uri, stamp, stamp, stamp);
  xml_escape(f, app_id());
  fputs("\" exec=\"&apos;", f);
  xml_escape(f, app_id());
  fprintf(f,
          " %%u&apos;\" modified=\"%s\" count=\"1\"/>\n"
          "        </bookmark:applications>\n"
          "      </metadata>\n"
          "    </info>\n"
          "  </bookmark>\n"
          "</xbel>\n",
          stamp);
  const bool ok = fclose(f) == 0 && rename(tmp, file) == 0;
  if (!ok) {
    remove(tmp);
  }
  free(uri);
  free(old);
  return ok;
}

/* -- the backend -- */

int bk_shell_os_features(void)
{
  int f = BK_SHELL_RECENT;
  if (connect_bus()) {
    f |= BK_SHELL_NOTIFY | BK_SHELL_PROGRESS;
    if (g_watcher) {
      f |= BK_SHELL_TRAY;
    }
    bk_shell_poll(); /* what the blocking calls above read */
  }
  return f;
}

void bk_shell_os_set_app_id(const char *id)
{
  (void)id; /* read when used (app_id) */
}

bool bk_shell_os_tray_add(BKTray *t)
{
  TrayDBus *os = (TrayDBus *)calloc(1, sizeof(TrayDBus));
  if (!os || !g_conn) {
    free(os);
    return false;
  }
  snprintf(os->name, sizeof(os->name), "org.kde.StatusNotifierItem-%d-%d", (int)getpid(), t->uid);
  if (D.dbus_bus_request_name(g_conn, os->name, NAME_DO_NOT_QUEUE, NULL) != NAME_PRIMARY_OWNER) {
    free(os);
    return false;
  }
  os->argb = icon_argb(t->icon);
  os->revision = 1;
  t->os = os;
  register_tray(t);
  return true;
}

void bk_shell_os_tray_remove(BKTray *t)
{
  if (g_conn) { /* the watcher drops an item whose name goes */
    D.dbus_bus_release_name(g_conn, os_of(t)->name, NULL);
  }
  free(os_of(t)->argb);
  free(t->os);
  t->os = NULL;
}

bool bk_shell_os_tray_update(BKTray *t, int changed)
{
  TrayDBus *os = os_of(t);
  bool ok = true;
  if (!g_conn) {
    return false;
  }
  if (changed & BK_TRAY_ICON) {
    unsigned char *argb = icon_argb(t->icon);
    ok = argb || !t->icon; /* a file that isn't an image keeps the old icon */
    if (ok) {
      free(os->argb);
      os->argb = argb;
      signal_tray(t, "NewIcon");
    }
  }
  if (changed & BK_TRAY_TOOLTIP) {
    signal_tray(t, "NewToolTip");
    signal_tray(t, "NewTitle");
  }
  if (changed & BK_TRAY_MENU) {
    char path[48];
    snprintf(path, sizeof(path), MENU_PATH "%d", t->uid);
    os->revision++;
    DBusMessage *m = D.dbus_message_new_signal(path, MENU_IFACE, "LayoutUpdated");
    if (m) {
      DBusMessageIter it;
      const int32_t parent = 0;
      D.dbus_message_iter_init_append(m, &it);
      put(&it, T_UINT, &os->revision);
      put(&it, T_INT, &parent);
    }
    send_msg(m);
  }
  return ok;
}

bool bk_shell_os_notify(int id, const char *title, const char *body, const char *icon)
{
  if (!g_conn) {
    return false;
  }
  DBusMessage *m = D.dbus_message_new_method_call(NOTIFY_NAME, NOTIFY_PATH, NOTIFY_NAME,
                                                  "Notify");
  if (!m) {
    return false;
  }
  /* (s app, u replaces, s icon, s summary, s body, as actions, a{sv} hints, i timeout) */
  DBusMessageIter it, arr, dict, e, var, st, bytes;
  const uint32_t replaces = 0;
  const int32_t timeout = -1;
  D.dbus_message_iter_init_append(m, &it);
  put_str(&it, app_id());
  put(&it, T_UINT, &replaces);
  put_str(&it, "");
  put_str(&it, title);
  put_str(&it, body);
  D.dbus_message_iter_open_container(&it, T_ARRAY, "s", &arr);
  put_str(&arr, "default"); /* the click */
  put_str(&arr, "Open");
  D.dbus_message_iter_close_container(&it, &arr);
  D.dbus_message_iter_open_container(&it, T_ARRAY, "{sv}", &dict);
  entry_begin(&dict, &e, "desktop-entry");
  put_var_str(&e, app_id());
  entry_end(&dict, &e);
  BKImBuf *ib = icon ? bk_shell_icon(icon, 64, (const float[4]){1.0f, 1.0f, 1.0f, 1.0f}) : NULL;
  int w = 0, h = 0;
  unsigned char *rgba = ib && BK_imbuf_size(ib, &w, &h) ? (unsigned char *)malloc((size_t)w * h * 4) :
                                                          NULL;
  if (rgba && BK_imbuf_rgba(ib, 0, rgba)) { /* image-data (iiibiiay) */
    const int32_t stride = w * 4, bits = 8, channels = 4;
    const dbus_bool_t alpha = 1;
    entry_begin(&dict, &e, "image-data");
    D.dbus_message_iter_open_container(&e, T_VARIANT, "(iiibiiay)", &var);
    D.dbus_message_iter_open_container(&var, T_STRUCT, NULL, &st);
    put(&st, T_INT, &w);
    put(&st, T_INT, &h);
    put(&st, T_INT, &stride);
    put(&st, T_BOOL, &alpha);
    put(&st, T_INT, &bits);
    put(&st, T_INT, &channels);
    D.dbus_message_iter_open_container(&st, T_ARRAY, "y", &bytes);
    const unsigned char *p = rgba;
    D.dbus_message_iter_append_fixed_array(&bytes, T_BYTE, &p, w * h * 4);
    D.dbus_message_iter_close_container(&st, &bytes);
    D.dbus_message_iter_close_container(&var, &st);
    D.dbus_message_iter_close_container(&e, &var);
    entry_end(&dict, &e);
  }
  free(rgba);
  BK_imbuf_free(ib);
  D.dbus_message_iter_close_container(&it, &dict);
  put(&it, T_INT, &timeout);
  DBusPendingCall *pending = NULL;
  const bool ok = D.dbus_connection_send_with_reply(g_conn, m, &pending, -1) && pending;
  if (ok) {
    D.dbus_pending_call_set_notify(pending, notify_reply, (void *)(intptr_t)id, NULL);
  }
  D.dbus_message_unref(m);
  D.dbus_connection_flush(g_conn);
  return ok;
}

bool bk_shell_os_progress(BKWindow *win, int state, float value)
{
  (void)win; /* the launcher entry is the app's */
  if (!g_conn) {
    return false;
  }
  char uri[300];
  snprintf(uri, sizeof(uri), "application://%s.desktop", app_id());
  DBusMessage *m = D.dbus_message_new_signal("/com/canonical/unity/launcherentry/bui",
                                             "com.canonical.Unity.LauncherEntry", "Update");
  if (!m) {
    return false;
  }
  DBusMessageIter it, dict, e;
  const double progress = state == BK_PROGRESS_INDETERMINATE ? 0.0 : (double)value;
  D.dbus_message_iter_init_append(m, &it);
  put_str(&it, uri);
  D.dbus_message_iter_open_container(&it, T_ARRAY, "{sv}", &dict);
  entry_begin(&dict, &e, "progress");
  put_var(&e, "d", T_DOUBLE, &progress);
  entry_end(&dict, &e);
  entry_begin(&dict, &e, "progress-visible");
  put_var_bool(&e, state != BK_PROGRESS_NONE);
  entry_end(&dict, &e);
  entry_begin(&dict, &e, "urgent"); /* how an error shows there */
  put_var_bool(&e, state == BK_PROGRESS_ERROR);
  entry_end(&dict, &e);
  D.dbus_message_iter_close_container(&it, &dict);
  send_msg(m);
  return true;
}

bool bk_shell_os_add_recent(const char *path)
{
  return add_recent_xbel(path);
}

#endif
