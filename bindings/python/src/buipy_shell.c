/* The desktop shell from Python (kit/intern/shell, BK_tray_*, BK_notify, BK_shell_*): the same
 * tray icons, notifications and recent files every binding uses; KitWindow.set_progress is in
 * buipy_kit.c. The window loop delivers the events (with the GIL taken here).
 *
 *   tray = buipy.Tray(icon="BLENDER", tooltip="My app")
 *   tray.set_menu([MenuItem("Open", on_click=open_), None, MenuItem("Quit", on_click=quit_)])
 *   tray.on_click = lambda button: ...
 *   buipy.on_notification_click(lambda nid: ...)
 *   nid = buipy.notify("Done", "The export finished")
 *
 * SPDX-License-Identifier: GPL-2.0-or-later */

#include "buipy.h"

typedef struct TrayObject {
  PyObject_HEAD
  BKTray *tray;
  PyObject *on_click;        /* callable(button: int) or NULL */
  PyObject *on_double_click; /* callable() or NULL */
  struct TrayObject *next;   /* the live trays: the shell's events find theirs */
} TrayObject;

static TrayObject *g_trays = NULL;
static PyObject *g_on_notification = NULL; /* callable(id: int) or NULL */

static void call(PyObject *fn, PyObject *args)
{
  if (!fn || !args) {
    Py_XDECREF(args);
    return;
  }
  PyObject *r = PyObject_CallObject(fn, args);
  if (!r) {
    PyErr_WriteUnraisable(fn);
  }
  Py_XDECREF(r);
  Py_DECREF(args);
}

static void emit(void *data, BKTray *tray, int kind, void *ref, double a, double b)
{
  (void)data;
  (void)b;
  GIL_ENTER;
  TrayObject *t = g_trays;
  while (t && t->tray != tray) {
    t = t->next;
  }
  if (kind == BK_EV_CALL && ref) { /* a menu item's on_click */
    call((PyObject *)ref, PyTuple_New(0));
  }
  else if (kind == BK_EV_CLICK && t) {
    call(t->on_click, Py_BuildValue("(i)", (int)a));
  }
  else if (kind == BK_EV_DOUBLE && t) {
    call(t->on_double_click, PyTuple_New(0));
  }
  else if (kind == BK_EV_ACTIVATE && !tray) {
    call(g_on_notification, Py_BuildValue("(i)", (int)a));
  }
  GIL_LEAVE;
}

static void listen(void)
{
  static int done = 0;
  if (!done) {
    const BKShellCallbacks cb = {NULL, emit};
    BK_shell_set_callbacks(&cb);
    done = 1;
  }
}

/* -- Tray -- */

static void tray_close_native(TrayObject *self)
{
  if (!self->tray) {
    return;
  }
  TrayObject **p = &g_trays;
  while (*p && *p != self) {
    p = &(*p)->next;
  }
  if (*p) {
    *p = self->next;
  }
  BK_tray_free(self->tray);
  self->tray = NULL;
}

static int tray_init(TrayObject *self, PyObject *args, PyObject *kw)
{
  static char *kwlist[] = {"icon", "tooltip", NULL};
  const char *icon = NULL, *tooltip = "";
  if (!PyArg_ParseTupleAndKeywords(args, kw, "|zs:Tray", kwlist, &icon, &tooltip)) {
    return -1;
  }
  if (self->tray) {
    PyErr_SetString(PyExc_RuntimeError, "Tray already initialized");
    return -1;
  }
  listen();
  self->tray = BK_tray_new(icon, tooltip);
  if (!self->tray) {
    PyErr_SetString(PyExc_OSError, "no tray on this system (see buipy.shell_features())");
    return -1;
  }
  self->next = g_trays;
  g_trays = self;
  return 0;
}

static int tray_traverse(TrayObject *self, visitproc visit, void *arg)
{
  Py_VISIT(self->on_click);
  Py_VISIT(self->on_double_click);
  return 0;
}

static int tray_clear(TrayObject *self)
{
  Py_CLEAR(self->on_click);
  Py_CLEAR(self->on_double_click);
  return 0;
}

static void tray_dealloc(TrayObject *self)
{
  PyObject_GC_UnTrack(self);
  tray_close_native(self);
  tray_clear(self);
  Py_TYPE(self)->tp_free((PyObject *)self);
}

static BKTray *live(TrayObject *self)
{
  if (!self->tray) {
    PyErr_SetString(PyExc_RuntimeError, "the tray icon was closed");
  }
  return self->tray;
}

static PyObject *tray_set_icon(TrayObject *self, PyObject *arg)
{
  const char *icon = arg == Py_None ? NULL : PyUnicode_AsUTF8(arg);
  if ((arg != Py_None && !icon) || !live(self)) {
    return NULL;
  }
  return PyBool_FromLong(BK_tray_set_icon(self->tray, icon));
}

static PyObject *tray_set_tooltip(TrayObject *self, PyObject *arg)
{
  const char *tip = PyUnicode_AsUTF8(arg);
  if (!tip || !live(self)) {
    return NULL;
  }
  return PyBool_FromLong(BK_tray_set_tooltip(self->tray, tip));
}

static PyObject *tray_set_menu(TrayObject *self, PyObject *items)
{
  BKMenuSpec *specs;
  int n;
  if (!live(self) || !buipy_menu_specs(items, &specs, &n)) {
    return NULL;
  }
  const int ok = BK_tray_set_menu(self->tray, specs, n);
  buipy_menu_specs_free(specs, n);
  return PyBool_FromLong(ok);
}

static PyObject *tray_close(TrayObject *self, PyObject *unused)
{
  (void)unused;
  tray_close_native(self);
  Py_RETURN_NONE;
}

static PyMethodDef tray_methods[] = {
    {"set_icon", (PyCFunction)tray_set_icon, METH_O,
     "set_icon(icon) -> bool: an image file or a Blender icon's name; None: the app's icon."},
    {"set_tooltip", (PyCFunction)tray_set_tooltip, METH_O, "set_tooltip(text) -> bool"},
    {"set_menu", (PyCFunction)tray_set_menu, METH_O,
     "set_menu(items) -> bool: MenuItem-like objects (text, checked, enabled, on_click, "
     "submenu), None for a separator; [] removes it."},
    {"close", (PyCFunction)tray_close, METH_NOARGS, "Removes the icon."},
    {NULL, NULL, 0, NULL},
};

static PyObject *tray_get_cb(TrayObject *self, void *off)
{
  PyObject *v = *(PyObject **)((char *)self + (size_t)off);
  return Py_NewRef(v ? v : Py_None);
}

static int tray_set_cb(TrayObject *self, PyObject *v, void *off)
{
  PyObject **slot = (PyObject **)((char *)self + (size_t)off);
  if (v && v != Py_None && !PyCallable_Check(v)) {
    PyErr_SetString(PyExc_TypeError, "a callable or None");
    return -1;
  }
  Py_XSETREF(*slot, (v && v != Py_None) ? Py_NewRef(v) : NULL);
  return 0;
}

static PyGetSetDef tray_getset[] = {
    {"on_click", (getter)tray_get_cb, (setter)tray_set_cb,
     "called with the button (1 left, 2 middle, 3 right) when the icon is clicked",
     (void *)offsetof(TrayObject, on_click)},
    {"on_double_click", (getter)tray_get_cb, (setter)tray_set_cb,
     "called when the icon is double-clicked (Windows)",
     (void *)offsetof(TrayObject, on_double_click)},
    {NULL, NULL, NULL, NULL, NULL},
};

static PyTypeObject Tray_Type = {
    PyVarObject_HEAD_INIT(NULL, 0).tp_name = "buipy.Tray",
    .tp_basicsize = sizeof(TrayObject),
    .tp_flags = Py_TPFLAGS_DEFAULT | Py_TPFLAGS_HAVE_GC,
    .tp_doc = "Tray(icon=None, tooltip='') — a tray icon (the notification area, the menu bar, "
              "a panel's tray); OSError where the system has none.",
    .tp_new = PyType_GenericNew,
    .tp_init = (initproc)tray_init,
    .tp_dealloc = (destructor)tray_dealloc,
    .tp_traverse = (traverseproc)tray_traverse,
    .tp_clear = (inquiry)tray_clear,
    .tp_methods = tray_methods,
    .tp_getset = tray_getset,
};

/* -- functions -- */

static PyObject *py_features(PyObject *m, PyObject *unused)
{
  (void)m;
  (void)unused;
  return PyLong_FromLong(BK_shell_features());
}

static PyObject *py_set_app_id(PyObject *m, PyObject *arg)
{
  (void)m;
  const char *id = PyUnicode_AsUTF8(arg);
  if (!id) {
    return NULL;
  }
  BK_shell_set_app_id(id);
  Py_RETURN_NONE;
}

static PyObject *py_notify(PyObject *m, PyObject *args, PyObject *kw)
{
  (void)m;
  static char *kwlist[] = {"title", "body", "icon", NULL};
  const char *title, *body = "", *icon = NULL;
  if (!PyArg_ParseTupleAndKeywords(args, kw, "s|sz:notify", kwlist, &title, &body, &icon)) {
    return NULL;
  }
  listen();
  return PyLong_FromLong(BK_notify(title, body, icon));
}

static PyObject *py_on_notification_click(PyObject *m, PyObject *fn)
{
  (void)m;
  if (fn != Py_None && !PyCallable_Check(fn)) {
    PyErr_SetString(PyExc_TypeError, "a callable or None");
    return NULL;
  }
  listen();
  Py_XSETREF(g_on_notification, fn != Py_None ? Py_NewRef(fn) : NULL);
  Py_RETURN_NONE;
}

static PyObject *py_add_recent(PyObject *m, PyObject *arg)
{
  (void)m;
  PyObject *bytes = NULL;
  if (!PyUnicode_FSConverter(arg, &bytes)) {
    return NULL;
  }
  const int ok = BK_shell_add_recent(PyBytes_AS_STRING(bytes));
  Py_DECREF(bytes);
  return PyBool_FromLong(ok);
}

static PyMethodDef shell_functions[] = {
    {"shell_features", py_features, METH_NOARGS,
     "shell_features() -> int: SHELL_TRAY | SHELL_NOTIFY | SHELL_PROGRESS | SHELL_RECENT, what "
     "this build and system have."},
    {"set_app_id", py_set_app_id, METH_O,
     "set_app_id(id): Windows' AppUserModelID, the desktop file's id on Linux."},
    {"notify", (PyCFunction)(void (*)(void))py_notify, METH_VARARGS | METH_KEYWORDS,
     "notify(title, body='', icon=None) -> int: a desktop notification's id, 0 when it cannot "
     "show."},
    {"on_notification_click", py_on_notification_click, METH_O,
     "on_notification_click(fn): fn(id) when a notification is clicked; None: no listener."},
    {"add_recent_file", py_add_recent, METH_O,
     "add_recent_file(path) -> bool: adds the file to the system's recent documents."},
    {NULL, NULL, 0, NULL},
};

int buipy_shell_init(PyObject *module)
{
  if (PyType_Ready(&Tray_Type) < 0 ||
      PyModule_AddObjectRef(module, "Tray", (PyObject *)&Tray_Type) < 0 ||
      PyModule_AddFunctions(module, shell_functions) < 0)
  {
    return -1;
  }
  static const struct {
    const char *name;
    int value;
  } k_consts[] = {
      {"SHELL_TRAY", BK_SHELL_TRAY},
      {"SHELL_NOTIFY", BK_SHELL_NOTIFY},
      {"SHELL_PROGRESS", BK_SHELL_PROGRESS},
      {"SHELL_RECENT", BK_SHELL_RECENT},
      {"PROGRESS_NONE", BK_PROGRESS_NONE},
      {"PROGRESS_NORMAL", BK_PROGRESS_NORMAL},
      {"PROGRESS_INDETERMINATE", BK_PROGRESS_INDETERMINATE},
      {"PROGRESS_PAUSED", BK_PROGRESS_PAUSED},
      {"PROGRESS_ERROR", BK_PROGRESS_ERROR},
  };
  for (size_t i = 0; i < sizeof(k_consts) / sizeof(k_consts[0]); i++) {
    if (PyModule_AddIntConstant(module, k_consts[i].name, k_consts[i].value) < 0) {
      return -1;
    }
  }
  return 0;
}
