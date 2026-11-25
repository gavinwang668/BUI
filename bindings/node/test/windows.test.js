'use strict'
// `bui build` / `bui package` release options for Windows (windows.js): the GPU API marker and MSIX.
const test = require('node:test')
const assert = require('node:assert')
const zlib = require('node:zlib')
const { MARKER, MARKER_FIELD, patchGpuApi, isSigned, identity, manifest, solidPng } = require('../windows.js')

/** An "executable" with the kit's marker (and a look-alike that isn't one) inside. */
function fakeExe (api = 'd3d11') {
  const field = Buffer.alloc(MARKER_FIELD)
  field.write(api, 'latin1')
  return Buffer.concat([Buffer.from('MZ....'), MARKER, Buffer.from('zzz'), Buffer.from('..'), MARKER, field, Buffer.from('tail')])
}

test('--gpu rewrites the marker the kit compiles in, and nothing else', () => {
  const buf = fakeExe()
  const before = Buffer.from(buf)
  assert.strictEqual(patchGpuApi(buf, 'd3d12'), 1)
  const at = buf.lastIndexOf(MARKER) + MARKER.length
  assert.strictEqual(buf.subarray(at, at + MARKER_FIELD).toString('latin1').replace(/\0+$/, ''), 'd3d12')
  assert.strictEqual(buf.length, before.length)
  assert.ok(buf.subarray(0, buf.indexOf(MARKER) + MARKER.length + 3).equals(before.subarray(0, before.indexOf(MARKER) + MARKER.length + 3)),
    'a look-alike whose field is no API name stays')
  assert.strictEqual(patchGpuApi(buf, 'd3d11'), 1, 'and back')
  assert.ok(buf.equals(before))
})

test('--gpu takes only the APIs the kit has; a file without the marker has none to set', () => {
  assert.throws(() => patchGpuApi(fakeExe(), 'vulkan'), /d3d11 or d3d12/)
  assert.strictEqual(patchGpuApi(Buffer.from('no renderer here'), 'd3d12'), 0)
})

test('a file that is no signed PE image is not signed', () => {
  assert.strictEqual(isSigned(Buffer.from('MZ')), false)
  assert.strictEqual(isSigned(fakeExe()), false)
})

test('the MSIX identity is validated where it comes in', () => {
  assert.deepStrictEqual(identity({ name: 'my app!', version: '1.2' }),
    { name: 'my-app-', version: '1.2.0.0', publisher: 'CN=BUI App, OID.2.25.311729368913984317654407730594956997722=1' })
  assert.throws(() => identity({ name: 'ab' }), /3 to 50/)
  assert.throws(() => identity({ name: 'app', version: '1.2.3.4.5' }), /four numbers/)
  assert.throws(() => identity({ name: 'app', version: '1.x' }), /four numbers/)
  assert.throws(() => identity({ name: 'app', version: '70000' }), /0-65535/)
  assert.throws(() => identity({ name: 'app', publisher: 'Me' }), /CN=/)
})

test('the manifest escapes what the app is called and runs the executable full trust', () => {
  const id = identity({ name: 'app' })
  const m = manifest(id, { name: 'app', displayName: 'Tom & "Jerry" <3' }, 'app.exe')
  assert.match(m, /<DisplayName>Tom &amp; &quot;Jerry&quot; &lt;3<\/DisplayName>/)
  assert.match(m, /Executable="app.exe" EntryPoint="Windows.FullTrustApplication"/)
  assert.match(m, /<rescap:Capability Name="runFullTrust"\/>/)
})

test('the default logos are valid PNGs of the size asked for', () => {
  const png = solidPng(44, [1, 2, 3])
  assert.ok(png.subarray(0, 8).equals(Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])))
  assert.strictEqual(png.readUInt32BE(16), 44)
  assert.strictEqual(png.readUInt32BE(20), 44)
  const idat = png.indexOf('IDAT')
  const raw = zlib.inflateSync(png.subarray(idat + 4, idat + 4 + png.readUInt32BE(idat - 4)))
  assert.strictEqual(raw.length, 44 * (1 + 44 * 3))
  assert.deepStrictEqual([...raw.subarray(1, 4)], [1, 2, 3])
})
