// The desktop shell (kit/intern/shell, through blenderui::shell): tray icons with menus,
// notifications, a window's taskbar progress and the system's recent documents. Names follow
// Electron's (Tray, setContextMenu, setProgressBar, addRecentDocument) so apps port as they are.
//
// SPDX-License-Identifier: GPL-2.0-or-later
'use strict'

const TYPES = ['normal', 'checkbox', 'separator', 'submenu']
const MODES = ['normal', 'indeterminate', 'paused', 'error', 'none']

function needs (native, fn) {
  if (typeof native[fn] !== 'function') {
    throw new Error('blenderui: this runtime has no desktop shell (tray, notifications, progress)')
  }
}

/** A tray icon: `app.createTray({ icon, tooltip, menu })`. */
class Tray {
  constructor (app, id) {
    this.app = app
    this.id = id
    this._on = { click: new Set(), 'right-click': new Set(), 'middle-click': new Set(), 'double-click': new Set() }
    this._clicks = new Map() // item id -> click()
    this._destroyed = false
  }

  /** An image file or a Blender icon's name ('BLENDER'); null: the app's own icon. */
  setImage (icon) { this._live().native.traySetIcon(this.id, icon == null ? null : String(icon)) }

  setToolTip (text) { this._live().native.traySetTooltip(this.id, String(text ?? '')) }

  /**
   * Its menu, opened by a right click: Electron's menu template, `{ label, type: 'normal' |
   * 'checkbox' | 'separator' | 'submenu', checked, enabled, click, submenu }`. null removes it.
   */
  setContextMenu (template) {
    const app = this._live()
    const clicks = new Map() // the old menu's stay until the new one is valid
    const items = (template ?? []).map((it) => this._item(it, clicks))
    this._clicks = clicks
    app.native.traySetMenu(this.id, items)
  }

  /** 'click', 'right-click' (an icon without a menu), 'middle-click', 'double-click' (Windows). */
  on (event, fn) {
    const set = this._on[event]
    if (!set) throw new Error(`blenderui: unsupported tray event "${event}"`)
    set.add(fn)
    return this
  }

  destroy () {
    if (this._destroyed) return
    this._destroyed = true
    this.app._trays.delete(this.id)
    this.app.native.trayFree(this.id)
  }

  isDestroyed () { return this._destroyed }

  _live () {
    if (this._destroyed) throw new Error('blenderui: the tray was destroyed')
    return this.app
  }

  _item (it, clicks) {
    if (it === null || it === '-') it = { type: 'separator' }
    const type = it.type ?? (it.submenu ? 'submenu' : 'normal')
    if (!TYPES.includes(type)) throw new RangeError(`blenderui: unknown menu item type "${type}"`)
    const id = clicks.size + 1
    clicks.set(id, it.click)
    return {
      id,
      label: it.label == null ? undefined : String(it.label),
      type,
      checked: !!it.checked,
      enabled: it.enabled ?? true,
      submenu: type === 'submenu' ? (it.submenu ?? []).map((s) => this._item(s, clicks)) : undefined
    }
  }

  _event (raw) {
    if (raw.kind === 'traymenu') {
      const fn = this._clicks.get(raw.number)
      if (fn) call(fn, this)
      return
    }
    const name = raw.button === 0 ? 'double-click'
      : raw.button === 3 ? 'right-click' : raw.button === 2 ? 'middle-click' : 'click'
    for (const fn of this._on[name]) call(fn, this)
  }
}

function call (fn, arg) {
  try { fn(arg) } catch (err) { queueMicrotask(() => { throw err }) }
}

/** The shell's part of App (index.js mixes these in). */
const appShell = {
  /** A tray icon: resolves with it, or null where the system has none. */
  async createTray ({ icon = null, tooltip = '', menu = null } = {}) {
    needs(this.native, 'trayNew')
    const id = (this._nextTray = (this._nextTray ?? 0) + 1)
    if (!(await this.native.trayNew(id, icon == null ? null : String(icon), String(tooltip)))) return null
    const tray = new Tray(this, id)
    this._trays.set(id, tray)
    if (menu) tray.setContextMenu(menu)
    return tray
  },

  /** A desktop notification: resolves with its id, or null when it can't show. */
  async notify ({ title = '', body = '', icon = null, onClick = null } = {}) {
    needs(this.native, 'notify')
    const id = await this.native.notify(String(title), String(body), icon == null ? null : String(icon))
    if (id != null && onClick) this._notifications.set(id, onClick)
    return id
  },

  /** Adds a file to the system's recent documents (the jump list, Open Recent): resolves with whether it was added. */
  addRecentDocument (path) {
    needs(this.native, 'addRecentDocument')
    return this.native.addRecentDocument(String(path))
  },

  /** Windows' AppUserModelID, the desktop file's id on Linux ('org.example.App'). */
  setAppId (id) {
    needs(this.native, 'setAppId')
    this.native.setAppId(String(id))
  },

  /** What this build and system have: `{ tray, notifications, progress, recentDocuments }`. */
  shellFeatures () {
    if (typeof this.native.shellFeatures !== 'function') {
      return Promise.resolve({ tray: false, notifications: false, progress: false, recentDocuments: false })
    }
    return this.native.shellFeatures()
  },

  /** A shell event (tray, traymenu, notification); false for any other. */
  _shellEvent (raw) {
    if (raw.kind === 'tray' || raw.kind === 'traymenu') {
      this._trays.get(raw.target)?._event(raw)
      return true
    }
    if (raw.kind === 'notification') {
      const fn = this._notifications.get(raw.number)
      this._notifications.delete(raw.number)
      if (fn) call(fn, raw.number)
      return true
    }
    return false
  }
}

/**
 * Electron's `BrowserWindow.setProgressBar`: `progress` 0..1 on the taskbar button (Windows;
 * macOS: the Dock icon; Linux: the launcher entry); below 0 hides it. `mode`: 'normal',
 * 'indeterminate', 'paused', 'error' or 'none'.
 */
function setProgressBar (progress, { mode } = {}) {
  needs(this.app.native, 'setProgress')
  const value = Number(progress)
  const m = mode ?? (value < 0 ? 'none' : value > 1 ? 'indeterminate' : 'normal')
  if (!MODES.includes(m)) throw new RangeError(`blenderui: unknown progress mode "${m}"`)
  this.app.native.setProgress(this.id, m, Math.min(Math.max(value, 0), 1))
}

module.exports = { Tray, appShell, setProgressBar }
