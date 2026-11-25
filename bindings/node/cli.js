#!/usr/bin/env node
// bui: build standalone apps from a JavaScript / JSX entry.
//
//   bui build app.jsx [-o app.exe]   a small executable: BUI + QuickJS (~5 MB)
//   bui build app.jsx --bun          a Bun executable (full Node APIs, ~120 MB)
//   bui run app.jsx [args]           run on the small runtime (no executable)
//   bui bundle app.jsx -o main.js    one script for QuickJS, no runtime (Android: build.py --js)
//   bui package app.exe --msix       release any BUI executable (Rust, C ...) the same way
//
// Options:
//   -o, --out <file>   output file (default: the entry's name, .exe on Windows, .msix with
//                      --msix; bundle: main.js); never the entry itself
//   --bun              compile with `bun build --compile` instead of QuickJS
//   --console          keep a console window (Windows; default: none, for apps)
//   --dev              development builds (React's, Svelte's dev mode)
//   --features a,b     the kit features the app uses (default: package.json
//                      "bui": {"features": [...]}, else detected in the bundle);
//                      the smallest runtime with them is linked (features.js)
//   --gpu d3d11|d3d12  Windows: the Direct3D the app's GPU renderer starts with
//                      (default d3d11; BUI_GPU_API overrides it when it runs)
//   --msix             Windows: an MSIX package (install, Store) instead of a bare .exe;
//                      --name, --version (1.2.3.0), --display-name, --description,
//                      --publisher "CN=..", --publisher-name, --icon logo.png,
//                      --include <file|dir> (repeatable), --cert app.pfx [--cert-password p]
//                      (default: package.json's; unsigned without --cert)
//
// Bundling uses Bun (https://bun.sh), which must be installed.
// SPDX-License-Identifier: GPL-2.0-or-later
'use strict'
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { spawnSync } = require('node:child_process')
const features = require('./features.js')
const platform = require('./platform.js')
const windows = require('./windows.js')

const MAGIC = Buffer.from('BUIAPP01')
const { TAG, EXE } = platform

function fail (msg) {
  console.error(`bui: ${msg}`)
  process.exit(1)
}

function parse (argv) {
  const o = { cmd: argv[0], entry: null, out: null, bun: false, console: false, dev: false, features: null, rest: [],
    gpu: null, msix: false, pkg: { include: [] } }
  // MSIX identity options: --display-name -> pkg.displayName
  const pkgOpts = ['--name', '--version', '--publisher', '--publisher-name', '--display-name', '--description',
    '--icon', '--cert', '--cert-password']
  for (let i = 1; i < argv.length; i++) {
    const a = argv[i]
    const value = () => {
      if (i + 1 >= argv.length) fail(`${a} needs a value`)
      return argv[++i]
    }
    if (o.cmd === 'run' && o.entry !== null) o.rest.push(a)
    else if (a === '-o' || a === '--out') o.out = value()
    else if (a === '--bun') o.bun = true
    else if (a === '--console') o.console = true
    else if (a === '--dev') o.dev = true
    else if (a === '--features') o.features = value().split(',').map((s) => s.trim()).filter(Boolean)
    else if (a === '--gpu') o.gpu = value()
    else if (a === '--msix') o.msix = true
    else if (a === '--include') o.pkg.include.push(value())
    else if (pkgOpts.includes(a)) o.pkg[a.slice(2).replace(/-(.)/g, (_, c) => c.toUpperCase())] = value()
    else if (a.startsWith('-')) fail(`unknown option ${a}`)
    else o.entry = a
  }
  if (o.gpu !== null && !windows.GPU_APIS.includes(o.gpu)) fail(`--gpu takes ${windows.GPU_APIS.join(' or ')}, not "${o.gpu}"`)
  return o
}

/** The QuickJS runtime `name` (features.json runtimes; "full" is bui-runtime
 *  itself): `npm run build` here, this platform's package, or the workspace build's full one. */
function runtimePath (name = 'full') {
  const file = name === 'full' ? `bui-runtime.${TAG}${EXE}` : `bui-runtime-${name}.${TAG}${EXE}`
  for (const dir of [__dirname, platform.packageDir()]) {
    if (dir && fs.existsSync(path.join(dir, file))) return path.join(dir, file)
  }
  for (const profile of ['dist', 'release']) { // npm run build, or a plain cargo build
    const built = path.join(__dirname, '../rust/target', profile, `bui-runtime${EXE}`)
    if (name === 'full' && fs.existsSync(built)) return built
  }
  if (name !== 'full') return runtimePath('full') // a smaller one wasn't built: the full one works
  fail(`no bui-runtime for ${TAG}: install ${platform.packageName()} (an optional dependency), ` +
    'or run `npm run build` in bindings/node to build it')
}

/** The runtime an app needs: explicit features, else detected ones. */
function chooseRuntime (o, js) {
  try {
    const explicit = o.features ?? features.configured(o.entry)
    const needed = explicit ? new Set(explicit) : new Set(features.detect(js.toString('latin1')).keys())
    const rt = features.pickRuntime(needed)
    console.log(`bui: runtime "${rt.name}" for ${needed.size ? [...needed].join(', ') : 'no optional features'}${explicit ? '' : ' (detected)'}`)
    return rt.name
  } catch (e) {
    fail(e.message)
  }
}

/** Bun's executable: this process when running on Bun; otherwise asked once
 *  (npm installs it behind a shell shim on Windows), then run without a shell
 *  so arguments keep their quotes. */
let bunPath = null
function findBun () {
  if (bunPath !== null) return bunPath
  if (typeof Bun !== 'undefined') return (bunPath = process.execPath)
  const r = spawnSync('bun', ['-e', 'process.stdout.write(process.execPath)'],
    { encoding: 'utf8', shell: process.platform === 'win32' })
  if (r.status !== 0 || !r.stdout) fail('bundling needs Bun: https://bun.sh')
  return (bunPath = r.stdout.trim())
}

function bun (args, opts = {}) {
  const r = spawnSync(findBun(), args, { stdio: opts.quiet ? 'pipe' : 'inherit', encoding: 'utf8' })
  if (r.status !== 0) fail(`bun ${args[0]} failed${r.stderr ? `:
${r.stderr}` : ''}`)
  return r
}

/** Bundle with Bun (bundle.js: the framework bindings' plugins included). */
function bundleWith (options) {
  bun([path.join(__dirname, 'bundle.js'), JSON.stringify(options)], { quiet: true })
}

/** One script for QuickJS: the app and its packages, React in production mode. */
function bundle (entry, dev) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'bui-'))
  const out = path.join(dir, 'app.js')
  bundleWith({ entry: path.resolve(entry), out, dev })
  const js = fs.readFileSync(out)
  fs.rmSync(dir, { recursive: true, force: true })
  return js
}

/** Windows: mark the executable as a windowed app (no console window). */
function setGuiSubsystem (buf) {
  if (buf.readUInt16LE(0) !== 0x5a4d) return
  const pe = buf.readUInt32LE(0x3c)
  if (buf.readUInt32LE(pe) !== 0x4550) return
  buf.writeUInt16LE(2, pe + 24 + 68) // IMAGE_SUBSYSTEM_WINDOWS_GUI
}

/** The file to write: `-o`, else `fallback`; never the entry itself, which it would overwrite. */
function outPath (o, fallback) {
  const out = path.resolve(o.out ?? fallback)
  const same = process.platform === 'win32' || process.platform === 'darwin'
    ? out.toLowerCase() === path.resolve(o.entry).toLowerCase()
    : out === path.resolve(o.entry)
  if (same) fail(`the output would overwrite the entry ${o.entry}: pass another file with -o`)
  return out
}

/** What the nearest package.json above `file` says about the app (MSIX defaults). */
function projectInfo (file) {
  for (let dir = path.dirname(path.resolve(file)); ; dir = path.dirname(dir)) {
    const p = path.join(dir, 'package.json')
    if (fs.existsSync(p)) {
      try {
        const pkg = JSON.parse(fs.readFileSync(p, 'utf8'))
        return {
          name: pkg.name?.replace(/^@[^/]+\//, ''),
          version: pkg.version?.replace(/[-+].*$/, ''), // 1.2.3-beta: the numbers
          description: pkg.description
        }
      } catch { return {} }
    }
    if (path.dirname(dir) === dir) return {}
  }
}

/** A temporary folder for `f`, removed afterwards. */
function withTemp (f) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'bui-release-'))
  try {
    return f(dir)
  } finally {
    fs.rmSync(dir, { recursive: true, force: true })
  }
}

/** --gpu, then --msix, on `exe` (a file of this run's, changed in place); the package goes to `out`. */
function release (o, exe, out) {
  try {
    if (o.gpu) {
      const buf = fs.readFileSync(exe)
      if (windows.isSigned(buf)) console.warn('bui: the executable is signed; changing its GPU API breaks the signature (sign it again)')
      if (windows.patchGpuApi(buf, o.gpu) === 0) fail(`${path.basename(o.entry)} has no BUI GPU renderer to choose with --gpu`)
      fs.writeFileSync(exe, buf)
    }
    if (o.msix) {
      const info = projectInfo(o.entry)
      const name = path.basename(o.entry).replace(/\.[^.]+$/, '')
      const given = Object.fromEntries(Object.entries(o.pkg).filter(([, v]) => v !== undefined))
      const id = windows.msix(exe, out, { name, ...info, ...given })
      console.log(`bui: packaged ${path.relative(process.cwd(), out)} (${id.name} ${id.version}, ` +
        `${id.signed ? 'signed' : 'unsigned: Add-AppxPackage -AllowUnsigned installs it'})`)
    }
  } catch (e) {
    fail(e.message)
  }
}

function build (o) {
  const name = path.basename(o.entry).replace(/\.[^.]+$/, '')
  const out = outPath(o, o.msix ? `${name}.msix` : `${name}${EXE}`)
  const t0 = Date.now()
  withTemp((dir) => {
    const exe = o.msix ? path.join(dir, `${name}${EXE}`) : out
    if (o.bun) {
      bundleWith({ entry: path.resolve(o.entry), out: exe, bun: true, dev: o.dev, console: o.console || process.platform !== 'win32' })
    } else {
      const js = bundle(o.entry, o.dev)
      const runtime = fs.readFileSync(runtimePath(chooseRuntime(o, js)))
      if (process.platform === 'win32' && !o.console) setGuiSubsystem(runtime)
      const trailer = Buffer.alloc(16)
      MAGIC.copy(trailer, 0)
      trailer.writeBigUInt64LE(BigInt(js.length), 8)
      fs.writeFileSync(exe, Buffer.concat([runtime, js, trailer]))
      if (process.platform !== 'win32') fs.chmodSync(exe, 0o755)
    }
    release(o, exe, out)
  })
  const mb = (fs.statSync(out).size / 1048576).toFixed(1)
  console.log(`bui: built ${path.relative(process.cwd(), out)} (${mb} MB, ${o.bun ? 'Bun' : 'QuickJS'}) in ${Date.now() - t0} ms`)
}

/** `bui package app.exe`: --gpu / --msix on an executable built any other way (Rust, C ...). */
function packageExe (o) {
  if (!o.gpu && !o.msix) fail('package needs --msix, --gpu or both')
  const name = path.basename(o.entry).replace(/\.[^.]+$/, '')
  const out = outPath(o, o.msix ? `${name}.msix` : `${name}-${o.gpu}${EXE}`)
  withTemp((dir) => {
    const exe = path.join(dir, path.basename(o.entry)) // the input is never changed
    fs.copyFileSync(o.entry, exe)
    release(o, exe, out)
    if (!o.msix) fs.copyFileSync(exe, out)
  })
  if (!o.msix) console.log(`bui: wrote ${path.relative(process.cwd(), out)} (starts on ${o.gpu})`)
}

function run (o) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'bui-run-'))
  const file = path.join(dir, `${path.basename(o.entry).replace(/\.[^.]+$/, '')}.js`)
  fs.writeFileSync(file, bundle(o.entry, o.dev))
  const r = spawnSync(runtimePath(), [file, ...o.rest], { stdio: 'inherit' })
  fs.rmSync(dir, { recursive: true, force: true })
  process.exit(r.status ?? 1)
}

const o = parse(process.argv.slice(2))
if (!['build', 'run', 'bundle', 'package'].includes(o.cmd) || !o.entry) {
  const lines = fs.readFileSync(__filename, 'utf8').split('\n')
  const end = lines.findIndex((l) => l.startsWith('// Bundling uses Bun')) + 1 // the header comment
  console.log(lines.slice(1, end).map((l) => l.replace(/^\/\/ ?/, '')).join('\n'))
  process.exit(o.cmd ? 1 : 0)
}
if (!fs.existsSync(o.entry)) fail(`no such file: ${o.entry}`)
if (o.cmd === 'bundle') fs.writeFileSync(outPath(o, 'main.js'), bundle(o.entry, o.dev))
else if (o.cmd === 'build') build(o)
else if (o.cmd === 'package') packageExe(o)
else run(o)
