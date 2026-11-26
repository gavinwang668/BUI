'use strict'
// `bui build`'s feature detection and runtime choice (features.js, features.json).
const test = require('node:test')
const assert = require('node:assert')
const { detect, pickRuntime } = require('../features.js')

const names = (js) => [...detect(js).keys()].filter((n) => n !== 'freetype').sort()

test('a bundle without media needs no optional feature: the base runtime', () => {
  const js = 'jsx("div",{className:"p-4",children:jsx("button",{onClick:f,children:"Hi"})})'
  assert.deepStrictEqual(names(js), [])
  assert.strictEqual(pickRuntime(detect(js)).name, 'base')
})

test('media files in string literals and <video> are detected', () => {
  const js = 'jsx("img",{src:"logo.webp"});jsx("img",{src:`photos/a.JPG`});x("bg.png?v=2")' +
    ';jsx("video",{src:p})'
  assert.deepStrictEqual(names(js), ['jpeg', 'png', 'video', 'webp'])
  assert.strictEqual(pickRuntime(detect(js)).name, 'media')
})

test('the desktop shell is detected in the app\'s calls, not in the library\'s own code', () => {
  // the library defines `notify (` and calls `this.native.notify(`: every bundle has both
  const library = 'class App { async notify ({ title }) { return this.native.notify(String(title)) }' +
    ' async createTray () {} addRecentDocument (p) { return this.native.addRecentDocument(p) } }' +
    ' Window.prototype.setProgressBar = setProgressBar; function setProgressBar (p) {}'
  assert.deepStrictEqual(names(library), [])
  assert.strictEqual(pickRuntime(detect(library)).name, 'base')
  for (const call of ['app.createTray({})', 'await app.notify({ title: "x" })', 'win.setProgressBar(0.5)', 'app.addRecentDocument(p)']) {
    assert.deepStrictEqual(names(library + ';' + call), ['shell'], call)
    assert.strictEqual(pickRuntime(detect(library + ';' + call)).name, 'full', call)
  }
})

test('the GPU and the file browser need the full runtime', () => {
  assert.strictEqual(pickRuntime(new Set(['gpu'])).name, 'full') // explicit: a gpu option is in every bundle's library code
  assert.strictEqual(pickRuntime(new Set(['filebrowser'])).name, 'full')
})

test('explicit features are checked against the table', () => {
  assert.throws(() => pickRuntime(new Set(['jpeg2000'])), /unknown feature "jpeg2000"/)
  assert.strictEqual(pickRuntime(new Set(['png', 'video'])).name, 'media')
  assert.strictEqual(pickRuntime(new Set(['avif', 'heic'])).name, 'media')
})
