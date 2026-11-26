// The one place that defines the docs' sections and their order.
//
// A page joins a section through its frontmatter (`section: "Rust"`), and the
// sidebar, breadcrumbs, prev/next links and search all follow this order.
// To add a section, add it here; to add a page, add an .mdx file.
// `icon` is any Phosphor icon name (https://phosphoricons.com).
// A language's section has `lang` instead: its Catppuccin file icon (<LangIcon>).

export const sections = [
  { label: "Start here", blurb: "What BUI is and how to install it", icon: "compass" },
  { label: "Tutorials", blurb: "Build a first app, one language at a time", icon: "graduation-cap" },
  { label: "Python", blurb: "tkinter, Qt syntax and Blender components", lang: "python" },
  { label: "Rust", blurb: "HTML, CSS, Tailwind, rsx! and hooks, natively", lang: "rust" },
  { label: "JavaScript & React", blurb: "Node.js, Bun, React, Svelte, Vue and standalone apps", lang: "javascript" },
  { label: "Dear ImGui", blurb: "The real Dear ImGui, in Blender's theme", lang: "cpp" },
  { label: "C & C++", blurb: "The C kit and embedding BUI in your host", lang: "c" },
  { label: "Lua", blurb: "The bui-lua host and the bui module", lang: "lua" },
  { label: "D", blurb: "A DUB package with raw C declarations and D wrappers", lang: "d" },
  { label: "C#", blurb: "C# components over every kit function", lang: "csharp" },
  { label: "Go", blurb: "The kit from Go, and MyGo-style views", lang: "go" },
  { label: "Ruby", blurb: "A C extension gem, with interfaces built from blocks", lang: "ruby" },
  { label: "Concepts", blurb: "How BUI works under the hood", icon: "lightbulb-filament" },
  { label: "Performance", blurb: "Numbers, and how to measure them yourself", icon: "gauge" },
  { label: "Reference", blurb: "Variables, options, matrices and terms", icon: "books" },
  { label: "Project", blurb: "Roadmap, contributing and credits", icon: "flag-banner" },
] as const;

export type SectionLabel = (typeof sections)[number]["label"];

export const sectionLabels = sections.map((s) => s.label) as unknown as [SectionLabel, ...SectionLabel[]];

/**
 * Languages a page can be about, and the labels <LangTabs> understands.
 * `id` is what gets stored when a reader picks a tab; `label` is what <Tab label="..."> uses.
 */
export const languages = [
  { id: "python", label: "Python" },
  { id: "rust", label: "Rust" },
  { id: "javascript", label: "JavaScript" },
  { id: "c", label: "C" },
  { id: "cpp", label: "C++" },
  { id: "imgui", label: "Dear ImGui" },
  { id: "d", label: "D" },
  { id: "csharp", label: "C#" },
  { id: "go", label: "Go" },
  { id: "ruby", label: "Ruby" },
] as const;

export type LanguageId = (typeof languages)[number]["id"];
export const languageIds = languages.map((l) => l.id) as unknown as [LanguageId, ...LanguageId[]];

/**
 * The header's product-level links. They name areas of the site, not pages: the sidebar
 * lists the pages. `match` names the docs sections the link is current for; a page of its own
 * (an addon, a docs page with `page:` in its frontmatter) says so itself.
 */
export const topLinks = [
  { label: "Docs", href: "docs/start/introduction/", match: {} },
  { label: "Tutorials", href: "docs/tutorials/first-app-python/", match: { sections: ["Tutorials"] } },
  {
    label: "Addons",
    href: "addons/",
    match: {},
    // shown on hover or focus; the mobile menu lists them under the link
    menu: [
      { label: "Panels", href: "panels/", icon: "squares-four", blurb: "Tabbed, docked and floating panel layouts" },
      { label: "Charts", href: "charts/", icon: "chart-bar", blurb: "80 charts, with the math and a live render" },
    ],
  },
  { label: "Benchmarks", href: "benchmarks/", match: { sections: ["Performance"] } },
  { label: "Roadmap", href: "roadmap/", match: {} },
] as const;

export type TopLink = (typeof topLinks)[number];

/** Which top link is current for a docs page. */
export function currentTopLink(section: string): string {
  return topLinks.find((l) => (l.match as { sections?: readonly string[] }).sections?.includes(section))?.label ?? "Docs";
}
