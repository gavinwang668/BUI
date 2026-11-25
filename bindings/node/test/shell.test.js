'use strict'
// The desktop shell from JavaScript (shell.js): a tray icon with a menu, a notification, the
// taskbar progress and the recent documents, and the checks at the boundary. What the system
// lacks (app.shellFeatures()) is skipped, and only that. The events themselves are the C test's
// (tests/shell_test.c); here, a `traymenu` event as the UI thread sends it runs the item's click.
const test = require('node:test')
const assert = require('node:assert')
const path = require('node:path')
const { App, Tray } = require('..')

test('tray, menu, notification, progress and recent documents', async (t) => {
  const app = new App({ headless: true })
  t.after(() => app.quit())
  const features = await app.shellFeatures()
  assert.deepStrictEqual(Object.keys(features).sort(), ['notifications', 'progress', 'recentDocuments', 'tray'])
  if (features.tray) {
    const tray = await app.createTray({ icon: 'BLENDER', tooltip: 'node shell test' })
    assert.ok(tray instanceof Tray)
    let chosen = null
    tray.setContextMenu([
      { label: 'Open', click: () => { chosen = 'open' } },
      '-',
      { label: 'Mute', type: 'checkbox', checked: true },
      { label: 'More', submenu: [{ label: 'A', click: () => { chosen = 'a' } }] }
    ])
    assert.throws(() => tray.setContextMenu([{ label: 'x', type: 'radio' }]), RangeError)
    // the UI thread's event for item "A" (ids: Open 1, separator 2, Mute 3, More 4, A 5)
    app._event({ kind: 'traymenu', target: tray.id, number: 5 })
    assert.strictEqual(chosen, 'a')
    let clicks = 0
    tray.on('click', () => clicks++)
    app._event({ kind: 'tray', target: tray.id, button: 1 })
    assert.strictEqual(clicks, 1)
    assert.throws(() => tray.on('hover', () => {}), /unsupported tray event/)
    tray.setToolTip('node shell test: tooltip')
    tray.setImage(path.join(__dirname, '..', '..', '..', 'tests', 'images', 'quad.png'))
    tray.destroy()
    assert.ok(tray.isDestroyed())
    assert.throws(() => tray.setToolTip('gone'), /destroyed/)
  }
  if (features.notifications) {
    let clicked = null
    const id = await app.notify({ title: 'blenderui', body: 'A notification from the Node test', onClick: (n) => { clicked = n } })
    assert.ok(id > 0)
    app._event({ kind: 'notification', number: id })
    assert.strictEqual(clicked, id)
  }
  assert.strictEqual(await app.addRecentDocument('no/such/file.txt'), false)
  const win = app.window({ width: 200, height: 100 })
  win.setProgressBar(0.5) // headless: no taskbar button, nothing to show
  assert.throws(() => win.setProgressBar(0.5, { mode: 'sideways' }), RangeError)
})
