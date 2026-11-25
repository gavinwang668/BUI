"""The desktop shell from Python (buipy.Tray, notify, add_recent_file, KitWindow.set_progress):
a tray icon with a bui MenuItem menu, a notification, the checks at the boundary. The parts this
build or system lacks (buipy.shell_features()) are skipped, and only those.

SPDX-License-Identifier: GPL-2.0-or-later
"""

import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

import buipy  # noqa: E402
from bui.components.menus import MenuItem  # noqa: E402

IMAGES = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "tests", "images")


def main():
    features = buipy.shell_features()
    print("shell features", features)
    assert buipy.add_recent_file(os.path.join(IMAGES, "no-such-file.png")) is False
    if features & buipy.SHELL_TRAY:
        tray = buipy.Tray(icon="BLENDER", tooltip="buipy shell test")
        assert tray.set_tooltip("buipy shell test: tooltip")
        assert tray.set_icon(os.path.join(IMAGES, "quad.png"))
        assert not tray.set_icon(os.path.join(IMAGES, "no-such-image.png"))
        assert tray.set_icon(None)
        chosen = []
        assert tray.set_menu([
            MenuItem("Open", lambda: chosen.append("open")),
            None,
            MenuItem("Mute", lambda: chosen.append("mute"), checked=True),
            MenuItem("More", submenu=[MenuItem("A", lambda: chosen.append("a"))]),
        ])
        tray.on_click = lambda button: chosen.append(button)
        try:
            tray.on_click = 3
        except TypeError:
            pass
        else:
            raise AssertionError("on_click takes a callable")
        tray.close()
        try:
            tray.set_tooltip("closed")
        except RuntimeError:
            pass
        else:
            raise AssertionError("a closed tray is refused")
    else:
        print("no tray: skipped")
    if features & buipy.SHELL_NOTIFY:
        buipy.on_notification_click(lambda nid: None)
        assert buipy.notify("buipy", "A notification from the Python test") > 0
        buipy.on_notification_click(None)
    assert buipy.notify("") == 0
    print("ok")


if __name__ == "__main__":
    main()
