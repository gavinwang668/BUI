// Windows release options for a built BUI executable: the GPU API it starts with, and an MSIX
// package (the installable, Store-ready form of a Win32 app).
//
// The GPU API is a marker the kit compiles in (gpu/intern: "BUIGPUAPI=" and the API name, padded
// with NULs to a fixed width): patching it changes the app's default; BUI_GPU_API still overrides
// it at run time, and an app that picks one in code keeps its pick.
// SPDX-License-Identifier: GPL-2.0-or-later
'use strict'
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const zlib = require('node:zlib')
const { spawnSync } = require('node:child_process')

const GPU_APIS = ['d3d11', 'd3d12']
const MARKER = Buffer.from('BUIGPUAPI=')
/** Bytes after the marker the kit reserves for the API name, NUL-padded: BUI_GPU_API_MARKER_LEN (32,
 *  gpu/include/GPU_raster.h) less the marker's 10. */
const MARKER_FIELD = 22

/** Set the GPU API `api` is built to start with in executable `buf` (patched in place). Returns how
 *  many markers it changed; 0 means the executable has no BUI GPU renderer to choose. */
function patchGpuApi (buf, api) {
  if (!GPU_APIS.includes(api)) throw new Error(`--gpu takes ${GPU_APIS.join(' or ')}, not "${api}"`)
  let n = 0
  for (let at = buf.indexOf(MARKER); at >= 0; at = buf.indexOf(MARKER, at + MARKER.length)) {
    const field = buf.subarray(at + MARKER.length, at + MARKER.length + MARKER_FIELD)
    const name = field.toString('latin1').replace(/\0+$/, '')
    if (!GPU_APIS.includes(name)) continue // the bytes happened to match: not the kit's marker
    field.fill(0)
    field.write(api, 'latin1')
    n++
  }
  return n
}

/** Whether PE image `buf` carries an Authenticode signature (patching it would break it). */
function isSigned (buf) {
  if (buf.length < 0x40 || buf.readUInt16LE(0) !== 0x5a4d) return false
  const pe = buf.readUInt32LE(0x3c)
  if (buf.readUInt32LE(pe) !== 0x4550) return false
  const opt = pe + 24
  const dirs = opt + (buf.readUInt16LE(opt) === 0x20b ? 112 : 96) // PE32+ : PE32
  return buf.readUInt32LE(dirs + 4 * 8 + 4) !== 0 // the certificate table's size
}

// -- MSIX -----------------------------------------------------------------------------------------

/** The package identity from `opts`, validated as the manifest schema requires. */
function identity (opts) {
  const name = opts.name.replace(/[^A-Za-z0-9.-]/g, '-').replace(/^[.-]+/, '').slice(0, 50)
  if (name.length < 3) throw new Error(`--name "${opts.name}": an MSIX name needs 3 to 50 of A-Z a-z 0-9 . -`)
  const v = String(opts.version ?? '1.0.0').split('.').map((p) => Number(p))
  while (v.length < 4) v.push(0)
  if (v.length > 4 || v.some((p) => !Number.isInteger(p) || p < 0 || p > 65535)) {
    throw new Error(`--version "${opts.version}": four numbers 0-65535 (1.2.3.0)`)
  }
  // without a certificate: the publisher Windows installs unsigned packages from
  // (Add-AppxPackage -AllowUnsigned); with one it must be the certificate's subject
  const publisher = opts.publisher ?? 'CN=BUI App, OID.2.25.311729368913984317654407730594956997722=1'
  if (!/^CN=/.test(publisher)) throw new Error(`--publisher "${publisher}": a distinguished name (CN=...)`)
  return { name, version: v.join('.'), publisher }
}

const xml = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')

function manifest (id, opts, exe) {
  const arch = { x64: 'x64', arm64: 'arm64', ia32: 'x86' }[process.arch] ?? 'x64'
  const display = xml(opts.displayName ?? opts.name)
  return `<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
         xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
         xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
         IgnorableNamespaces="uap rescap">
  <Identity Name="${xml(id.name)}" Publisher="${xml(id.publisher)}" Version="${id.version}" ProcessorArchitecture="${arch}"/>
  <Properties>
    <DisplayName>${display}</DisplayName>
    <PublisherDisplayName>${xml(opts.publisherName ?? id.publisher.replace(/^CN=([^,]*).*$/, '$1'))}</PublisherDisplayName>
    <Logo>Assets\\StoreLogo.png</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.17763.0" MaxVersionTested="10.0.22621.0"/>
  </Dependencies>
  <Resources>
    <Resource Language="en-us"/>
  </Resources>
  <Applications>
    <Application Id="App" Executable="${xml(exe)}" EntryPoint="Windows.FullTrustApplication">
      <uap:VisualElements DisplayName="${display}" Description="${xml(opts.description ?? opts.displayName ?? opts.name)}"
                          BackgroundColor="transparent"
                          Square150x150Logo="Assets\\Square150x150Logo.png" Square44x44Logo="Assets\\Square44x44Logo.png"/>
    </Application>
  </Applications>
  <Capabilities>
    <rescap:Capability Name="runFullTrust"/>
  </Capabilities>
</Package>
`
}

/** A `size` x `size` PNG of one colour (the logos an app without an icon gets). */
function solidPng (size, rgb) {
  const crcTable = Array.from({ length: 256 }, (_, n) => {
    let c = n
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
    return c >>> 0
  })
  const crc = (b) => {
    let c = 0xffffffff
    for (const x of b) c = crcTable[(c ^ x) & 0xff] ^ (c >>> 8)
    return (c ^ 0xffffffff) >>> 0
  }
  const chunk = (type, data) => {
    const t = Buffer.concat([Buffer.from(type, 'latin1'), data])
    const len = Buffer.alloc(4)
    len.writeUInt32BE(data.length)
    const sum = Buffer.alloc(4)
    sum.writeUInt32BE(crc(t))
    return Buffer.concat([len, t, sum])
  }
  const ihdr = Buffer.alloc(13)
  ihdr.writeUInt32BE(size, 0)
  ihdr.writeUInt32BE(size, 4)
  ihdr[8] = 8 // bit depth
  ihdr[9] = 2 // RGB
  const row = Buffer.alloc(1 + size * 3)
  for (let x = 0; x < size; x++) row.set(rgb, 1 + x * 3)
  const raw = Buffer.concat(Array.from({ length: size }, () => row))
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(raw)), chunk('IEND', Buffer.alloc(0))])
}

/** A Windows SDK tool (makeappx, signtool): the newest SDK's, for this machine's architecture. */
function sdkTool (exe) {
  const root = path.join(process.env['ProgramFiles(x86)'] ?? 'C:\\Program Files (x86)', 'Windows Kits', '10', 'bin')
  const arch = process.arch === 'arm64' ? 'arm64' : 'x64'
  const versions = fs.existsSync(root) ? fs.readdirSync(root).filter((d) => /^10\./.test(d)) : []
  versions.sort((a, b) => a.localeCompare(b, undefined, { numeric: true })).reverse()
  for (const v of versions) {
    const p = path.join(root, v, arch, exe)
    if (fs.existsSync(p)) return p
  }
  throw new Error(`${exe} not found: install the Windows SDK (its "MSIX Packaging Tools")`)
}

function run (tool, args) {
  const r = spawnSync(tool, args, { encoding: 'utf8' })
  if (r.status !== 0) throw new Error(`${path.basename(tool)} failed:\n${(r.stdout ?? '') + (r.stderr ?? '')}`)
}

/** Package executable `exe` (and `opts.include`: more files or folders next to it) as `out`, an
 *  MSIX. `opts`: name, version, publisher, publisherName, displayName, description, icon (a PNG
 *  for every logo), cert / certPassword (a .pfx to sign with; its subject is the publisher). */
function msix (exe, out, opts) {
  if (process.platform !== 'win32') throw new Error('--msix packages Windows apps: run it on Windows')
  const id = identity(opts)
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'bui-msix-'))
  try {
    const exeName = path.basename(exe)
    fs.copyFileSync(exe, path.join(dir, exeName))
    for (const extra of opts.include ?? []) {
      fs.cpSync(extra, path.join(dir, path.basename(extra)), { recursive: true })
    }
    const assets = path.join(dir, 'Assets')
    fs.mkdirSync(assets)
    for (const [file, size] of [['StoreLogo.png', 50], ['Square150x150Logo.png', 150], ['Square44x44Logo.png', 44]]) {
      if (opts.icon) fs.copyFileSync(opts.icon, path.join(assets, file))
      else fs.writeFileSync(path.join(assets, file), solidPng(size, [0x47, 0x72, 0xb3]))
    }
    fs.writeFileSync(path.join(dir, 'AppxManifest.xml'), manifest(id, opts, exeName))
    fs.rmSync(out, { force: true })
    run(sdkTool('makeappx.exe'), ['pack', '/o', '/d', dir, '/p', out])
    if (opts.cert) {
      const sign = ['sign', '/fd', 'SHA256', '/f', opts.cert]
      if (opts.certPassword) sign.push('/p', opts.certPassword)
      run(sdkTool('signtool.exe'), [...sign, out])
    }
  } finally {
    fs.rmSync(dir, { recursive: true, force: true })
  }
  return { ...id, signed: Boolean(opts.cert) }
}

module.exports = { GPU_APIS, MARKER, MARKER_FIELD, patchGpuApi, isSigned, identity, manifest, solidPng, msix }
