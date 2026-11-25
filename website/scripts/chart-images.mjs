// Chart previews for /charts: the first example of each chart from the gallery's screenshots
// (`NNN-<chart>-<example>.png`, numbered in gallery order), resized to WebP in public/charts/images.
//   node scripts/chart-images.mjs <screenshots folder>
import { mkdirSync, readdirSync, readFileSync } from "node:fs"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"
import sharp from "sharp"

const site = join(dirname(fileURLToPath(import.meta.url)), "..")
const from = process.argv[2]
if (!from) throw new Error("usage: node scripts/chart-images.mjs <screenshots folder>")
const out = join(site, "public", "charts", "images")
mkdirSync(out, { recursive: true })

// the gallery numbers every example of every chart, Dashboards (one example each) first
const gallery = readFileSync(join(site, "..", "addons", "bui-charts", "examples", "gallery", "registry.rs"), "utf8")
const dashboards = [...gallery.matchAll(/^\s+Dashboards:/gm)].length
const registry = JSON.parse(readFileSync(join(site, "src", "data", "charts", "generated", "registry.json"), "utf8"))
const files = new Map(readdirSync(from).filter((f) => /^\d+-.*\.png$/.test(f)).map((f) => [Number.parseInt(f, 10), f]))

let index = dashboards + 1
const missing = []
for (const chart of registry) {
  const file = files.get(index)
  index += chart.examples
  // the screenshot names drop accented letters (`poincar-map`, `st-ve`), so only their start is compared
  if (!file || file.slice(4, 6) !== chart.slug.slice(0, 2)) {
    missing.push(`${chart.slug}${file ? ` (found ${file})` : ""}`)
    continue
  }
  await sharp(join(from, file)).resize(800, 450).webp({ quality: 78, alphaQuality: 90, effort: 6 }).toFile(join(out, `${chart.slug}.webp`))
}
console.log(`${registry.length - missing.length} previews`)
if (missing.length) console.log("no screenshot:", missing.join(", "))
