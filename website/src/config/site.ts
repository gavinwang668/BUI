// Site-wide settings. Change the URL here (or with SITE_URL at build time).

export const site = {
  name: "BUI",
  title: "BUI: Blender's UI, Decoupled out of it's codebase.",
  description:
    "BUI is a native UI toolkit built on the engine that draws Blender's interface: use Blender's widgets, plain HTML and CSS, or both. Drive it from tkinter, Qt-style Python, Blender components, Rust, JavaScript (React, Svelte, Vue), Dear ImGui, C, C++, C#, Go, Lua, Ruby or D.",
  repo: "https://github.com/ppmpreetham/BUI",
  /** Base for "Edit this page" links: a page's file path (src/content/...) follows it. */
  editBase: "https://github.com/ppmpreetham/BUI/edit/main/website/",
  /** Base for links to files in the repository (use `repoFile()` in content). */
  blobBase: "https://github.com/ppmpreetham/BUI/blob/main/",
  pypi: "https://pypi.org/project/blenderui/",
  npm: "https://www.npmjs.com/package/@blender-ui/react",
  crates: "https://crates.io/crates/blenderui",
  discord: "https://discord.gg/MWXZfdyM3D",
  license: "GPL-3.0-or-later",
} as const;
