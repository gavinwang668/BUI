// The Catppuccin file icons the site draws with <LangIcon>: one per SVG in src/assets/lang-icons
// (Catppuccin's VS Code icons, v1.26.0, MIT; add one by copying `icons/css-variables/<name>.svg`
// from that release and its name here).

export const langIconNames = [
  "python",
  "rust",
  "javascript",
  "javascript-react",
  "svelte",
  "vue",
  "html",
  "cpp",
  "c",
  "csharp",
  "d",
  "lua",
  "go",
  "ruby",
  "powershell",
  "npm",
  "bun",
  "cargo",
  "cmake",
  "lua-rocks",
  "go-mod",
  "nuget",
  "ruby-gem",
  "dub",
] as const;

export type LangIconName = (typeof langIconNames)[number];

/** The icon a docs <Tab label="…"> shows beside its label; labels not listed show none. */
export const tabIcons: Readonly<Record<string, LangIconName>> = {
  Python: "python",
  Rust: "rust",
  "rsx!": "rust",
  "gpui builders": "rust",
  JavaScript: "javascript",
  React: "javascript-react",
  Svelte: "svelte",
  Vue: "vue",
  C: "c",
  "C++": "cpp",
  "Dear ImGui": "cpp",
  "C#": "csharp",
  D: "d",
  Lua: "lua",
  Go: "go",
  Ruby: "ruby",
  PowerShell: "powershell",
};
