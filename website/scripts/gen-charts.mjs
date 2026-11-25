import { readFileSync, writeFileSync, mkdirSync } from "node:fs"
import { join, dirname } from "node:path"
import { fileURLToPath } from "node:url"
import { Marked } from "marked"
import { createHighlighter } from "shiki"

const here = dirname(fileURLToPath(import.meta.url))
const site = join(here, "..")
const charts = process.env.BUI_CHARTS ?? join(site, "..", "addons", "bui-charts")
const gallery = join(charts, "examples", "gallery")
const out = join(site, "src", "data", "charts", "generated")
mkdirSync(out, { recursive: true })

const slugOf = (name) =>
  name
    .normalize("NFD")
    .replace(/\p{M}/gu, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "")
const norm = (text) => slugOf(text).replace(/-/g, "")
const read = (path) => readFileSync(path, "utf8").replace(/\r\n/g, "\n")

const highlighter = await createHighlighter({ themes: ["github-light", "github-dark"], langs: ["rust", "bash", "text"] })
const themes = { light: "github-light", dark: "github-dark" }
const highlight = (code, lang = "rust") =>
  highlighter.codeToHtml(code, { lang: highlighter.getLoadedLanguages().includes(lang) ? lang : "text", themes, defaultColor: false })

const main = read(join(gallery, "registry.rs"))
const entries = [...main.matchAll(/^\s+(Base|Charts):\s+"([^"]+)"\s+=>\s+crate::([\w:]+)::EXAMPLES/gm)].map((m) => {
  const parts = m[3].split("::")
  return { group: m[1] === "Base" ? "base" : "derived", name: m[2], slug: slugOf(m[2]), file: parts[0], mod: parts[1] ?? null }
})

const readme = read(join(charts, "README.md"))
const tagInfo = new Map()
const sections = readme.split(/^#### /m)
sections.forEach((section, index) => {
  const heading = index === 0 ? "Core charts" : section.split("\n")[0].trim()
  const blurb = index === 0 ? "" : section.split("\n").slice(1).join(" ").split("<table>")[0].trim()
  for (const m of section.matchAll(/<a href="docs\/([\w-]+)\/examples\.md(#[\w-]+)?"><img [^>]*alt="(\w+)"/g)) {
    tagInfo.set(norm(m[3]), { tag: m[3], dir: m[1], blurb, family: heading, multi: Boolean(m[2]) })
  }
})

const docsReadme = read(join(charts, "docs", "README.md"))
for (const m of docsReadme.matchAll(/^\| (`<[^|]+) \| ([^|]+) \| \[Guide\]/gm)) {
  for (const tag of [...m[1].matchAll(/`<(\w+)>`/g)].map((t) => t[1])) {
    const info = tagInfo.get(norm(tag))
    if (info && !info.blurb) info.blurb = m[2].trim()
  }
}

const moduleBlock = (source, mod) => {
  if (!mod) return source
  const start = source.indexOf(`pub mod ${mod} {`)
  if (start < 0) return ""
  // a module ends at its closing brace, or where the next one starts (`}}` closes `examples!` and the module)
  const ends = [source.indexOf("\n}\n", start), source.indexOf("\npub mod ", start + 1)].filter((at) => at >= 0)
  const end = ends.length ? Math.min(...ends) : -1
  return source.slice(start, end < 0 ? undefined : end)
}

const dedent = (lines) => {
  const pad = Math.min(...lines.filter((l) => l.trim()).map((l) => l.match(/^ */)[0].length))
  return lines.map((l) => l.slice(Math.min(pad, l.match(/^ */)[0].length))).join("\n")
}

const snake = (tag) => tag.replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase()

const functionCode = (block, name) => {
  const lines = block.split("\n")
  const flat = (text) => text.replace(/_/g, "")
  const at = lines.findIndex((l) => {
    const m = l.match(/^\s*fn (\w+)\(/)
    return m && flat(m[1]) === flat(name)
  })
  if (at < 0) return null
  const indent = lines[at].match(/^ */)[0]
  let end = at
  while (end < lines.length && lines[end] !== `${indent}}`) end++
  const first = lines[at - 1]?.trim() === "#[component]" ? at - 1 : at
  return dedent(lines.slice(first, end + 1))
}

const macroCode = (block, name) => {
  const start = block.search(new RegExp(`^\\s*\\w+!\\(\\s*${name},`, "m"))
  if (start < 0) return null
  const end = block.indexOf(");", start)
  return dedent(block.slice(start, end + 2).split("\n"))
}

const exampleList = (entry) => {
  const source = read(join(gallery, `${entry.file}.rs`))
  let block = moduleBlock(source, entry.mod)
  const wrapped = block.match(/^\s*(\w+)!\((\w+)\);/m)
  if (wrapped) {
    const macro = source.match(new RegExp(`macro_rules! ${wrapped[1]} \\{[\\s\\S]*?\\n\\}\\n`))
    if (macro) block = macro[0].replace(/\$tag/g, wrapped[2])
  }
  const body = block.match(/examples! \{([\s\S]*?)\n?\s*\}/)
  if (!body) return []
  return [...body[1].matchAll(/^\s*(\w+):\s+"((?:[^"\\]|\\.)*)"\s+=>\s+"((?:[^"\\]|\\.)*)"/gm)].map((m) => {
    const name = snake(m[1])
    const code = functionCode(block, name) ?? macroCode(block, name) ?? ""
    return { id: name, title: m[2].replace(/\\"/g, '"'), description: m[3].replace(/\\"/g, '"'), code: code ? highlight(code) : "" }
  })
}

const registry = new Map(entries.map((e) => [e.slug, e]))
const dirFirst = new Map()
for (const entry of entries) {
  const info = tagInfo.get(norm(entry.name)) ?? tagInfo.get(norm(entry.name.replace(/ Map$/, "")))
  entry.info = info
  if (info && !dirFirst.has(info.dir)) dirFirst.set(info.dir, entry.slug)
}
const tagToSlug = new Map(entries.filter((e) => e.info).map((e) => [norm(e.info.tag), e.slug]))

const rewriteLink = (href) => {
  const match = href.match(/^(?:\.\.\/([\w-]+)\/|)(api|guide|examples)\.md(?:#([\w-]+))?$/) ?? href.match(/^\.\.\/([\w-]+)\/(api|guide|examples)\.md(?:#([\w-]+))?$/)
  if (!match) return href
  const [, dir, , anchor] = match
  const slug = (anchor && tagToSlug.get(anchor)) || (dir && dirFirst.get(dir)) || null
  return slug ? `/charts/${slug}` : "/charts/all"
}

const marked = new Marked({
  async: true,
  walkTokens: async (token) => {
    if (token.type === "code") token.escaped = true, (token.text = highlight(token.text, token.lang || "rust"))
    if (token.type === "link") token.href = rewriteLink(token.href)
  },
  renderer: {
    code: ({ text }) => text,
    heading: function ({ tokens, depth }) {
      const text = tokens.map((t) => t.raw).join("")
      const id = slugOf(text.replace(/[`<>]/g, ""))
      return `<h${depth + 2} id="${id}">${this.parser.parseInline(tokens)}</h${depth + 2}>`
    },
    table: function ({ header, rows }) {
      const head = header.map((c) => `<th>${this.parser.parseInline(c.tokens)}</th>`).join("")
      const body = rows.map((r) => `<tr>${r.map((c) => `<td>${this.parser.parseInline(c.tokens)}</td>`).join("")}</tr>`).join("")
      return `<div class="api-table"><table><thead><tr>${head}</tr></thead><tbody>${body}</tbody></table></div>`
    },
  },
})

const apiSection = (entry) => {
  const info = entry.info
  if (!info) return ""
  const text = read(join(charts, "docs", info.dir, "api.md"))
  if (!info.multi) return text.replace(/^# .*\n/, "")
  const start = text.search(new RegExp(`^## .*<${info.tag}>`, "m"))
  if (start < 0) return ""
  const rest = text.slice(start)
  const next = rest.slice(3).search(/^## /m)
  return rest.slice(rest.indexOf("\n") + 1, next < 0 ? undefined : next + 3).replace(/\n---\s*$/, "")
}

const manifest = []
for (const entry of entries) {
  const api = await marked.parse(apiSection(entry))
  const examples = exampleList(entry)
  writeFileSync(join(out, `${entry.slug}.json`), JSON.stringify({ api, examples }))
  manifest.push({
    slug: entry.slug,
    name: entry.name,
    tag: entry.info?.tag ?? entry.name.replace(/\W/g, ""),
    group: entry.group,
    dir: entry.info?.dir ?? "",
    family: entry.info?.family ?? "",
    blurb: entry.info?.blurb ?? "",
    examples: examples.length,
  })
}
writeFileSync(join(out, "registry.json"), JSON.stringify(manifest, null, 2))
const snippets = JSON.parse(readFileSync(join(site, "src", "data", "charts", "snippets.json"), "utf8"))
writeFileSync(join(out, "snippets.json"), JSON.stringify(Object.fromEntries(Object.entries(snippets).map(([key, value]) => [key, highlight(value.code, value.lang)]))))
const bad = manifest.filter((m) => m.examples < 10 || !m.dir)
console.log(`${manifest.length} charts, ${manifest.reduce((n, m) => n + m.examples, 0)} examples`)
if (bad.length) console.log("check:", bad.map((m) => `${m.slug}(${m.examples})`).join(", "))
const empty = []
for (const m of manifest) {
  const data = JSON.parse(readFileSync(join(out, `${m.slug}.json`), "utf8"))
  const missing = data.examples.filter((e) => !e.code).length
  if (missing) empty.push(`${m.slug}:${missing}`)
  if (!data.api) empty.push(`${m.slug}:noapi`)
}
if (empty.length) console.log("missing code/api:", empty.join(", "))
