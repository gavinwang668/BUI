"""A desktop's side of the D-Bus shell protocols, for bui_shell_test on Linux (kit/intern/shell/
shell_dbus.c): a StatusNotifierWatcher that, when an item registers, reads it the way a panel does
(its properties, its dbusmenu layout) and clicks it and a menu item; a notification server that
"clicks" the shell test's first notification; and a listener for the launcher entry's progress.

    dbus-run-session -- sh -c 'python3 tests/shell_dbus_host.py & sleep 1; \
        BUI_SHELL_TEST_HOST=1 ./bui_shell_test'

Needs dbus-python and PyGObject. Prints what it checked; exits after a few seconds.

SPDX-License-Identifier: GPL-2.0-or-later
"""

import sys

import dbus
import dbus.mainloop.glib
import dbus.service
from gi.repository import GLib

dbus.mainloop.glib.DBusGMainLoop(set_as_default=True)
bus = dbus.SessionBus()
loop = GLib.MainLoop()
failures = []


def check(ok, what):
    print(("ok   " if ok else "FAIL ") + what, flush=True)
    if not ok:
        failures.append(what)


def labels(layout):
    """Every label of a dbusmenu layout (ia{sv}av), in pre-order, with ids."""
    out = []
    item_id, props, kids = layout
    if "label" in props or props.get("type") == "separator":
        out.append((int(item_id), str(props.get("label", "-"))))
    for kid in kids:
        out += labels(kid)
    return out


def inspect(service):
    item = bus.get_object(service, "/StatusNotifierItem")
    props = dbus.Interface(item, "org.freedesktop.DBus.Properties")
    sni = props.GetAll("org.kde.StatusNotifierItem")
    check(sni["Status"] == "Active", "item status")
    check(sni["Title"] == "BUI shell test: tooltip", "item title is the tooltip")
    pixmaps = sni["IconPixmap"]
    check(len(pixmaps) == 1 and pixmaps[0][0] == 32 and len(pixmaps[0][2]) == 32 * 32 * 4,
          "icon pixmap 32x32 ARGB")
    check(sni["ToolTip"][2] == "BUI shell test: tooltip", "tooltip")
    menu_path = str(sni["Menu"])
    menu = dbus.Interface(bus.get_object(service, menu_path), "com.canonical.dbusmenu")
    revision, layout = menu.GetLayout(0, -1, dbus.Array([], signature="s"))
    got = labels(layout)
    check(got == [(1, "Open"), (2, "-"), (3, "Check"), (4, "More"), (5, "A"), (6, "B"),
                  (7, "Quit")], "menu layout %s" % got)
    props = dict(menu.GetGroupProperties(dbus.Array([3, 6], signature="i"),
                                         dbus.Array([], signature="s")))
    check(props[3].get("toggle-state") == 1 and props[3].get("toggle-type") == "checkmark",
          "check item")
    check(props[6].get("enabled") is not None and not props[6]["enabled"], "disabled item")
    check(str(dbus.Interface(bus.get_object(service, menu_path),
                             "org.freedesktop.DBus.Properties").Get("com.canonical.dbusmenu",
                                                                    "Status")) == "normal",
          "menu status")
    dbus.Interface(item, "org.kde.StatusNotifierItem").Activate(0, 0)
    menu.Event(5, "clicked", dbus.Int32(0, variant_level=1), dbus.UInt32(0))
    return False


class Watcher(dbus.service.Object):
    @dbus.service.method("org.kde.StatusNotifierWatcher", in_signature="s", sender_keyword="sender")
    def RegisterStatusNotifierItem(self, service, sender=None):
        check(service.startswith("org.kde.StatusNotifierItem-"), "item registered: " + service)
        GLib.timeout_add(100, inspect, service)


class Notifications(dbus.service.Object):
    next_id = 0

    @dbus.service.method("org.freedesktop.Notifications", in_signature="susssasa{sv}i",
                         out_signature="u")
    def Notify(self, app, replaces, icon, summary, body, actions, hints, timeout):
        Notifications.next_id += 1
        nid = Notifications.next_id
        print("notification %d: %s / %s, actions %s, image %s" % (
            nid, summary, body, list(actions), "image-data" in hints), flush=True)
        check("default" in actions, "the notification's click action")
        if body == "A notification from the shell test":
            GLib.timeout_add(200, lambda: self.ActionInvoked(nid, "default") and False)
        return nid

    @dbus.service.method("org.freedesktop.Notifications", out_signature="as")
    def GetCapabilities(self):
        return ["actions", "body"]

    @dbus.service.signal("org.freedesktop.Notifications", signature="us")
    def ActionInvoked(self, nid, action):
        pass


def launcher(uri, props):
    print("launcher entry %s: progress %.2f visible %s" % (
        uri, float(props.get("progress", -1)), bool(props.get("progress-visible"))), flush=True)


watcher_name = dbus.service.BusName("org.kde.StatusNotifierWatcher", bus)
Watcher(bus, "/StatusNotifierWatcher")
notify_name = dbus.service.BusName("org.freedesktop.Notifications", bus)
Notifications(bus, "/org/freedesktop/Notifications")
bus.add_signal_receiver(launcher, signal_name="Update",
                        dbus_interface="com.canonical.Unity.LauncherEntry")
GLib.timeout_add(int(float(sys.argv[1]) * 1000) if len(sys.argv) > 1 else 6000, loop.quit)
loop.run()
print("host: %s" % ("FAILED %s" % failures if failures else "ok"), flush=True)
sys.exit(1 if failures else 0)
