// The landing page's "Write it the way you already write UI" picker. Every snippet is
// taken from the repository's README or docs (see `source`), so it matches what BUI runs.
//
// Screenshots: `shot` names a light/dark pair in public/images/shots/ (<shot>-light.png and
// <shot>-dark.png, listed in public/images/shots/INDEX.md). The screenshot is a fuller app in
// the same syntax as the snippet; `caption` names its source. `fallback` is only used if a
// pair goes missing; see src/lib/shots.ts.

import type { LangIconName } from "../../lib/lang-icons";

export interface Syntax {
  id: string;
  /** The syntax switcher's label. */
  label: string;
  /** Catppuccin file icon beside the label (<LangIcon>). */
  icon: LangIconName;
  title: string;
  blurb: string;
  lang: string;
  file: string;
  code: string;
  install: string;
  shot: string;
  fallback: string;
  alt: string;
  caption: string;
  docs: string;
  source: string;
}

export interface Language {
  id: string;
  tab: string;
  /** Catppuccin file icon (<LangIcon>). */
  icon: LangIconName;
  syntaxes: Syntax[];
}

const tkinter: Syntax = {
  id: "tkinter",
  label: "tkinter",
  icon: "python",
  title: "Your tkinter app, unchanged",
  blurb:
    "Every public tkinter and ttk name is there: 7,846 of 7,846. Run an existing program with blenderui-tk and it wakes up looking like Blender.",
  lang: "py",
  file: "greet.py",
  code: `import blenderui.tksyntax as tk
from blenderui.tksyntax import ttk, messagebox

root = tk.Tk()
name = tk.StringVar(value="Cube")
ttk.Entry(root, textvariable=name).pack(fill="x", padx=8, pady=8)
ttk.Button(root, text="Greet",
           command=lambda: messagebox.showinfo("Hi", "Hello " + name.get())).pack()
root.mainloop()`,
  install: "pip install blenderui && blenderui-tk my_app.py",
  shot: "python-tkinter",
  fallback: "images/shots/python-tkinter-dark.png",
  alt: "An unmodified tkinter program on BUI: a glTF export dialog with entries, check and radio buttons, a spinbox, an option menu, a scale and a listbox, drawn with Blender's widgets",
  caption: "examples/site/tk_export.py, unmodified tkinter",
  docs: "docs/python/tkinter/",
  source: "README.md",
};

const qt: Syntax = {
  id: "qt",
  label: "Qt syntax",
  icon: "python",
  title: "Qt-style Python, no Qt required",
  blurb: "QWidget, layouts, signals and clicked.connect, drawn by BUI. The everyday subset works today; more is on the way.",
  lang: "py",
  file: "form.py",
  code: `from blenderui.qtsyntax import (QApplication, QMainWindow, QWidget, QVBoxLayout,
                                QFormLayout, QLineEdit, QSlider, QProgressBar, Qt)

app = QApplication([])
win = QMainWindow()
central = QWidget()
layout = QVBoxLayout(central)

form = QFormLayout()
level = QSlider(Qt.Horizontal)
level.setRange(0, 100)
bar = QProgressBar()
level.valueChanged.connect(bar.setValue)   # signals connect to any callable
form.addRow("Name:", QLineEdit())
form.addRow("Level:", level)
form.addRow("Progress:", bar)
layout.addLayout(form)

win.setCentralWidget(central)
win.show()
app.exec()`,
  install: "pip install blenderui",
  shot: "python-qt",
  fallback: "images/shots/python-qt-dark.png",
  alt: "A Qt-syntax material editor on BUI: a list, a line edit, a group box with a form layout, a combo box, a slider with a spin box, a check box and a progress bar",
  caption: "examples/site/qt_material.py",
  docs: "docs/python/qt-syntax/",
  source: "docs/qt.md",
};

const rsx: Syntax = {
  id: "rsx",
  label: "rsx!",
  icon: "rust",
  title: "HTML, CSS and Tailwind, natively",
  blurb:
    "rsx!, components and hooks, all 113 HTML elements and modern CSS. Add <NumberSlider> and you get Blender's real slider. Typos in Tailwind classes fail to compile, politely.",
  lang: "rust",
  file: "src/main.rs",
  code: `use blenderui::prelude::*;

struct Hello;

impl Render for Hello {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        rsx! {
            <body m-0 p-6 flex flex-col gap-4 bg-zinc-900 text-white>
                <h1 text-2xl font-bold>"Hello, BUI"</h1>
                <NumberSlider min={0.0} max={10.0} value="5" />   // a real Blender slider
            </body>
        }
    }
}

fn main() {
    Application::new().run(|cx| {
        cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Hello)).unwrap();
    });
}`,
  install: "cargo add blenderui",
  shot: "rust-rsx-dashboard",
  fallback: "images/shots/rust-rsx-dashboard-dark.png",
  alt: "A render-farm dashboard written with rsx!: a CSS grid of stat cards, job rows with Blender progress bars, a node list, a Blender dropdown, button and checkbox",
  caption: "examples/site_dashboard.rs, rsx!",
  docs: "docs/rust/rsx-and-components/",
  source: "README.md",
};

const gpui: Syntax = {
  id: "gpui",
  label: "gpui builders",
  icon: "rust",
  title: "gpui code compiles unchanged",
  blurb:
    "The same file builds against Zed's gpui, gpui-fast and blenderui. This is the benchmark's settings window: on blenderui it shows its first frame in 40 ms instead of 231 ms.",
  lang: "rust",
  file: "settings.rs",
  code: `impl Render for Settings {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let field = |label: &'static str| {
            div().flex().items_center().gap(px(8.0))
                .child(div().w(px(70.0)).text_color(rgb(0xb8b8b8)).child(label))
                .child(div().flex_1().h(px(20.0)).px(px(6.0)).rounded(px(4.0)).bg(rgb(0x1d1d1d))
                    .border_1().border_color(rgb(0x3d3d3d)).text_color(rgb(0xe6e6e6)).child("Cube"))
        };
        div()
            .size_full().flex().flex_col().gap(px(8.0)).p(px(12.0))
            .bg(rgb(0x303030)).text_color(rgb(0xe6e6e6)).text_size(px(13.0))
            .child(div().text_size(px(16.0)).child("Settings"))
            .child(field("Name"))
            .child(div().w(px(80.0)).h(px(22.0)).rounded(px(4.0)).bg(rgb(0x545454))
                .flex().items_center().justify_center().child("Apply"))
    }
}`,
  install: "cargo add blenderui",
  shot: "rust-builder-dashboard",
  fallback: "images/shots/rust-builder-dashboard-dark.png",
  alt: "The same render-farm dashboard written with gpui-style builders, drawn with identical pixels",
  caption: "examples/site_dashboard_builder.rs, gpui builders",
  docs: "docs/rust/gpui-syntax/",
  source: "bindings/rust/bench/common/settings.rs",
};

const htmlBuilder: Syntax = {
  id: "html",
  label: "HTML builder",
  icon: "html",
  title: "Plain HTML elements, as functions",
  blurb:
    "One constructor per HTML element (h2(), ul(), input(), button() …) and gpui's app model. Elements with an id keep their state across renders; cx.notify() re-renders.",
  lang: "rust",
  file: "src/todo.rs",
  code: `use blenderui::prelude::*;

struct Todo { items: Vec<String>, draft: String }

impl Render for Todo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().p_4().flex().flex_col().gap_2()
            .child(h2().child(format!("{} items", self.items.len())))
            .child(ul().children(self.items.iter().map(|s| li().child(s.clone()))))
            .child(div().flex().gap_2()
                .child(input().id("draft").placeholder("New item").value(self.draft.clone())
                    .on_change(cx.listener(|this, e: &ChangeEvent, _, cx| {
                        this.draft = e.value.clone();
                        cx.notify();
                    })))
                .child(button().child("Add").on_click(cx.listener(|this, _, _, cx| {
                    this.items.push(std::mem::take(&mut this.draft));
                    cx.notify();
                }))))
    }
}`,
  install: "cargo add blenderui",
  shot: "rust-html-plain",
  fallback: "images/shots/rust-html-plain-dark.png",
  alt: "An export form and a table built from plain HTML element functions, with a browser's default look",
  caption: "examples/site_plain_html.rs, plain HTML",
  docs: "docs/rust/html-elements/",
  source: "docs/rust.md",
};

const react: Syntax = {
  id: "react",
  label: "React (JSX)",
  icon: "javascript-react",
  title: "React, without a browser",
  blurb:
    "React 19.3 renders straight to BUI on Node.js or Bun. Hooks keep React's names but hold signals, so a change can update one node without a re-render.",
  lang: "jsx",
  file: "todo.jsx",
  code: `/** @jsxImportSource @blender-ui/react */
import { render, useState, Button, Checkbox, ProgressBar, TextField } from "@blender-ui/react"
import { TASKS, TAG_COLORS, CSS } from "./todo_style"

function Task({ task, onToggle }) {
  return (
    <div className={task.done ? "item done" : "item"}>
      <Checkbox checked={task.done} onChange={onToggle} />
      <span className="text">{task.text}</span>
      <span className="tag" style={{ backgroundColor: TAG_COLORS[task.tag] }}>{task.tag}</span>
    </div>
  )
}

function Tasks() {
  const [tasks, setTasks] = useState(TASKS)
  const [filter, setFilter] = useState("All")
  const [draft, setDraft] = useState("")
  const all = tasks.value ?? tasks
  const done = all.filter((t) => t.done).length
  const pct = Math.round((100 * done) / all.length)
  const shown = all.filter((t) => filter.value === "All" || (filter.value === "Done") === t.done)
  return (
    <div>
      <style>{CSS}</style>
      <div className="head">
        <h1>Robot Arm · Sprint 4</h1>
        <span className="muted">{all.length - done} open · {done} done</span>
      </div>
      <div className="add">
        <TextField placeholder="Add a task…" value={draft} onChange={(e) => setDraft(e.target.value)} />
        <Button onClick={() => draft.value && setTasks((ts) => [...ts, { text: draft.value, tag: "Modeling", done: false }])}>Add</Button>
      </div>
      <div className="filters">
        {["All", "Active", "Done"].map((f) => (
          <span key={f} className={filter.value === f ? "filter on" : "filter"} onClick={() => setFilter(f)}>{f}</span>
        ))}
      </div>
      <div className="list">
        {shown.map((t) => (
          <Task key={t.text} task={t} onToggle={() => setTasks((ts) => ts.map((x) => (x === t ? { ...x, done: !x.done } : x)))} />
        ))}
      </div>
      <div className="foot">
        <ProgressBar className="bar" max="100" value={String(pct)} />
        <span className="muted">{pct}% complete</span>
      </div>
    </div>
  )
}

render(<Tasks />, { title: "Tasks (React)", width: 440, height: 370, gpu: true })`,
  install: "npm install @blender-ui/react react",
  shot: "js-react-tasks",
  fallback: "images/shots/js-react-tasks-dark.png",
  alt: "A task list in React on BUI: a Blender text field and button, checkboxes and a progress bar",
  caption: "examples/site/js/todo_react.js, React on BUI",
  docs: "docs/javascript/react/",
  source: "examples/site/js/todo_react.js",
};

const svelte: Syntax = {
  id: "svelte",
  label: "Svelte",
  icon: "svelte",
  title: "Svelte 5, in a native window",
  blurb:
    "Runes, snippets, bindings and scoped styles, unchanged. Svelte's compiled components run on BUI's DOM, so a $state change updates the one node that shows it.",
  lang: "svelte",
  file: "Tasks.svelte",
  code: `<script>
  import { Button, Checkbox, ProgressBar, TextField } from "@blender-ui/svelte"
  import { TASKS, TAG_COLORS, CSS } from "./todo_style"

  let tasks = $state(TASKS.map((t) => ({ ...t })))
  let filter = $state("All")
  let draft = $state("")
  const done = $derived(tasks.filter((t) => t.done).length)
  const pct = $derived(Math.round((100 * done) / tasks.length))
  const shown = $derived(tasks.filter((t) => filter === "All" || (filter === "Done") === t.done))
</script>

<div>
  <svelte:element this={"style"}>{CSS}</svelte:element>
  <div class="head">
    <h1>Robot Arm · Sprint 4</h1>
    <span class="muted">{tasks.length - done} open · {done} done</span>
  </div>
  <div class="add">
    <TextField placeholder="Add a task…" value={draft} oninput={(e) => (draft = e.target.value)} />
    <Button onclick={() => draft && tasks.push({ text: draft, tag: "Modeling", done: false })}>Add</Button>
  </div>
  <div class="filters">
    {#each ["All", "Active", "Done"] as f (f)}
      <span class={filter === f ? "filter on" : "filter"} onclick={() => (filter = f)}>{f}</span>
    {/each}
  </div>
  <div class="list">
    {#each shown as task (task.text)}
      <div class={task.done ? "item done" : "item"}>
        <Checkbox checked={task.done} onchange={() => (task.done = !task.done)} />
        <span class="text">{task.text}</span>
        <span class="tag" style:background-color={TAG_COLORS[task.tag]}>{task.tag}</span>
      </div>
    {/each}
  </div>
  <div class="foot">
    <ProgressBar class="bar" max="100" value={String(pct)} />
    <span class="muted">{pct}% complete</span>
  </div>
</div>

<!-- app.js: render(Tasks, { title: "Tasks", gpu: true }) -->`,
  install: "npm install @blender-ui/svelte svelte",
  shot: "js-svelte-tasks",
  fallback: "images/shots/js-svelte-tasks-dark.png",
  alt: "A task list in Svelte on BUI: a Blender text field and button, checkboxes and a progress bar",
  caption: "examples/site/js/TodoSvelte.svelte, Svelte on BUI",
  docs: "docs/javascript/svelte/",
  source: "examples/site/js/TodoSvelte.svelte",
};

const vue: Syntax = {
  id: "vue",
  label: "Vue",
  icon: "vue",
  title: "Vue 3, in a native window",
  blurb:
    "Refs, v-model, slots and scoped styles, unchanged. Vue's own runtime and compiler run through a custom renderer over BUI's DOM, so a ref change updates the nodes that show it.",
  lang: "vue",
  file: "Tasks.vue",
  code: `<script setup>
import { ref, computed } from "vue"
import { Button, Checkbox, ProgressBar, TextField } from "@blender-ui/vue"
import { TASKS, TAG_COLORS, CSS } from "./todo_style"

let nextId = 0
const tasks = ref(TASKS.map((t) => ({ ...t, id: nextId++ })))
const filter = ref("All")
const draft = ref("")
const done = computed(() => tasks.value.filter((t) => t.done).length)
const pct = computed(() => Math.round((100 * done.value) / tasks.value.length))
const shown = computed(() => tasks.value.filter((t) => filter.value === "All" || (filter.value === "Done") === t.done))
const add = () => { if (draft.value) tasks.value.push({ id: nextId++, text: draft.value, tag: "Modeling", done: false }) }
</script>

<template>
  <div>
    <component :is="'style'">{{ CSS }}</component>
    <div class="head">
      <h1>Robot Arm · Sprint 4</h1>
      <span class="muted">{{ tasks.length - done }} open · {{ done }} done</span>
    </div>
    <div class="add">
      <TextField placeholder="Add a task…" v-model="draft" />
      <Button @click="add">Add</Button>
    </div>
    <div class="filters">
      <span v-for="f in ['All', 'Active', 'Done']" :key="f" :class="filter === f ? 'filter on' : 'filter'" @click="filter = f">{{ f }}</span>
    </div>
    <div class="list">
      <div v-for="task in shown" :key="task.id" :class="task.done ? 'item done' : 'item'">
        <Checkbox v-model="task.done" />
        <span class="text">{{ task.text }}</span>
        <span class="tag" :style="{ backgroundColor: TAG_COLORS[task.tag] }">{{ task.tag }}</span>
      </div>
    </div>
    <div class="foot">
      <ProgressBar class="bar" max="100" :value="String(pct)" />
      <span class="muted">{{ pct }}% complete</span>
    </div>
  </div>
</template>

<!-- app.js: render(Tasks, { title: "Tasks", gpu: true }) -->`,
  install: "npm install @blender-ui/vue vue",
  shot: "js-vue-tasks",
  fallback: "images/shots/js-vue-tasks-dark.png",
  alt: "A task list in Vue on BUI: a Blender text field and button, checkboxes and a progress bar",
  caption: "examples/site/js/TodoVue.vue, Vue on BUI",
  docs: "docs/javascript/vue/",
  source: "examples/site/js/TodoVue.vue",
};

const dom: Syntax = {
  id: "dom",
  label: "DOM-like API",
  icon: "javascript",
  title: "A DOM you already know, minus the browser",
  blurb:
    "createElement, appendChild, addEventListener and className with Tailwind. Any attribute can be a signal: bind it once and the node follows.",
  lang: "js",
  file: "todo.js",
  code: `const { App } = require("@blender-ui/native")
const { TASKS, TAG_COLORS, CSS } = require("./todo_style")

const app = new App({ gpu: true })
const win = app.window({ title: "Tasks", width: 440, height: 370 })
const el = (tag, cls, text) => {
  const e = win.createElement(tag)
  if (cls) e.className = cls
  if (text !== undefined) e.appendChild(win.createTextNode(String(text)))
  return e
}
win.body.setAttribute("styled", "")
win.body.appendChild(el("style", null, CSS))

const head = el("div", "head")
head.appendChild(el("h1", null, "Robot Arm · Sprint 4"))
const count = el("span", "muted")
head.appendChild(count)
win.body.appendChild(head)

const add = el("div", "add")
const input = win.createElement("TextField")
input.setAttribute("placeholder", "Add a task…")
add.appendChild(input)
const addBtn = el("Button", null, "Add")
add.appendChild(addBtn)
win.body.appendChild(add)

const filters = el("div", "filters")
for (const [i, f] of ["All", "Active", "Done"].entries()) filters.appendChild(el("span", i === 0 ? "filter on" : "filter", f))
win.body.appendChild(filters)

const list = el("div", "list")
win.body.appendChild(list)
const foot = el("div", "foot")
const progress = win.createElement("ProgressBar")
progress.className = "bar"
progress.setAttribute("max", "100")
foot.appendChild(progress)
const pct = el("span", "muted")
foot.appendChild(pct)
win.body.appendChild(foot)

const tasks = TASKS.map((t) => ({ ...t }))
const refresh = () => {
  for (const c of [...list.childNodes]) { list.removeChild(c); c.destroy() }
  for (const t of tasks) {
    const row = el("div", t.done ? "item done" : "item")
    const box = win.createElement("Checkbox")
    if (t.done) box.setAttribute("checked", "")
    box.addEventListener("change", (e) => { t.done = e.target.checked; refresh() })
    row.appendChild(box)
    row.appendChild(el("span", "text", t.text))
    const tag = el("span", "tag", t.tag)
    tag.style = { backgroundColor: TAG_COLORS[t.tag] }
    row.appendChild(tag)
    list.appendChild(row)
  }
  const done = tasks.filter((t) => t.done).length
  count.textContent = \`\${tasks.length - done} open · \${done} done\`
  progress.setAttribute("value", String(Math.round((100 * done) / tasks.length)))
  pct.textContent = \`\${Math.round((100 * done) / tasks.length)}% complete\`
}
addBtn.addEventListener("click", () => {
  if (input.value) { tasks.push({ text: input.value, tag: "Modeling", done: false }); refresh() }
})
refresh()`,
  install: "npm install @blender-ui/native",
  shot: "js-dom-tasks",
  fallback: "images/shots/js-dom-tasks-dark.png",
  alt: "The same task list built with createElement, appendChild and addEventListener",
  caption: "examples/site/js/todo_dom.js, @blender-ui/native",
  docs: "docs/javascript/native-api/",
  source: "examples/site/js/todo_dom.js",
};

const imguiCpp: Syntax = {
  id: "cpp",
  label: "C++",
  icon: "cpp",
  title: "The real Dear ImGui, in Blender's clothes",
  blurb:
    "Existing ImGui code compiles unchanged. Flip one build option (BUI_IMGUI_BLENDER_WIDGETS) and DragFloat becomes Blender's drag-to-scrub field.",
  lang: "cpp",
  file: "imgui_demo.cpp",
  code: `#include "imgui.h"
#include "imgui_impl_bui.h"

static void frame(void*) { ImGui::ShowDemoWindow(); }

int main() {
  ImGui::CreateContext();
  ImGui_ImplBUI_App* app = ImGui_ImplBUI_CreateApp("Dear ImGui on BUI", 1280, 800, frame, nullptr);
  ImGui_ImplBUI_RunApp(app);       // BUI's message loop until the window closes
  ImGui_ImplBUI_DestroyApp(app);
  ImGui::DestroyContext();
}`,
  install: "cmake -S . -B build -DBUI_IMGUI_BLENDER_WIDGETS=ON && cmake --build build",
  shot: "imgui-cpp-blender",
  fallback: "images/shots/imgui-cpp-blender-dark.png",
  alt: "A Dear ImGui scene inspector drawn with Blender's widgets and style: a table, drag fields, a color edit, sliders, a combo, checkboxes, a progress bar and a plot",
  caption: "examples/site/imgui_inspector.cpp, BUI_IMGUI_BLENDER_WIDGETS=ON",
  docs: "docs/imgui/cpp/",
  source: "docs/imgui.md",
};

const imguiC: Syntax = {
  id: "c",
  label: "C",
  icon: "c",
  title: "All 399 functions, from C",
  blurb: "cimgui conventions: ImGui::Button is ImGui_Button, references become pointers, every struct keeps its C++ layout.",
  lang: "c",
  file: "imgui_demo_c.c",
  code: `#include "cimgui.h"
#include "cimgui_impl_bui.h"

static void frame(void* user)
{
    float* value = (float*)user;
    ImGui_ShowDemoWindow();
    ImGui_Begin("Hello");
    ImGui_SliderFloat("Value", value, 0.0f, 1.0f);
    ImGui_Text("%.2f", *value);
    ImGui_End();
}

int main(void)
{
    float value = 0.5f;
    ImGui_CreateContext();
    cImGui_ImplBUI_App* app = cImGui_ImplBUI_CreateApp("Hello", 1280, 800, frame, &value, NULL);
    cImGui_ImplBUI_RunApp(app);
    cImGui_ImplBUI_DestroyApp(app);
    ImGui_DestroyContext();
}`,
  install: "cmake -S . -B build && cmake --build build --target bui_imgui_demo_c",
  shot: "imgui-cpp-stock",
  fallback: "images/shots/imgui-cpp-stock-dark.png",
  alt: "The same Dear ImGui scene inspector with ImGui's own widgets and its StyleColorsDark / StyleColorsLight colors",
  caption: "ImGui's own widgets and colors (examples/site/imgui_inspector.cpp --stock)",
  docs: "docs/imgui/c/",
  source: "docs/imgui.md",
};

const imguiPy: Syntax = {
  id: "python",
  label: "Python",
  icon: "python",
  title: "pyimgui-style Python",
  blurb: "snake_case functions; an in/out value comes back after the result: changed, v = imgui.slider_float(…).",
  lang: "py",
  file: "hello_imgui.py",
  code: `import blenderui.imgui as imgui

state = {"value": 0.5}

def frame():
    imgui.show_demo_window()
    imgui.begin("Hello")
    _, state["value"] = imgui.slider_float("Value", state["value"], 0.0, 1.0)
    imgui.end()

imgui.run(frame, title="Hello", size=(1280, 800))`,
  install: "pip install blenderui",
  shot: "imgui-python-blender",
  fallback: "images/shots/imgui-python-blender-dark.png",
  alt: "The Dear ImGui scene inspector from Python, with Blender's widgets and style",
  caption: "examples/site/imgui_inspector.py, blenderui.imgui",
  docs: "docs/imgui/python/",
  source: "docs/imgui.md",
};

const imguiRust: Syntax = {
  id: "rust",
  label: "Rust",
  icon: "rust",
  title: "A safe Rust API, and the raw FFI",
  blurb: "&mut values for in/out arguments, &str for strings, slices for (pointer, count) pairs. imgui::sys has every C function.",
  lang: "rust",
  file: "src/main.rs",
  code: `use blenderui_imgui as imgui;

fn main() {
    let mut value = 0.5f32;
    imgui::run("Hello", [1280, 800], move || {
        imgui::show_demo_window();
        imgui::begin("Hello");
        imgui::slider_float("Value", &mut value, 0.0, 1.0);
        if imgui::button("Reset") {
            value = 0.5;
        }
        imgui::text(&format!("value {value:.2}"));
        imgui::end();
    });
}`,
  install: "cargo run -p blenderui-imgui --example demo",
  shot: "imgui-python-stock",
  fallback: "images/shots/imgui-python-stock-dark.png",
  alt: "The Dear ImGui scene inspector with ImGui's own widgets and colors",
  caption: "The scene inspector, ImGui's own style (examples/site/imgui_inspector.py)",
  docs: "docs/imgui/rust/",
  source: "docs/imgui.md",
};

const c: Syntax = {
  id: "kit",
  label: "C kit",
  icon: "c",
  title: "The kit itself, in C",
  blurb: "Everything above sits on one C runtime. Build a node tree, set properties, and open a native window.",
  lang: "c",
  file: "hello.c",
  code: `#include "BUI_kit.h"

int main(void)
{
  BKNode *root = BK_node_new("frame");
  BKNode *size = BK_node_new("slider");
  BK_set_str(size, "label", "Size");
  BK_set_num(size, "max", 10);
  BK_set_num(size, "value", 2.5);
  BK_node_add(root, size, -1);              /* -1: append */
  BK_geom_set_manager(size, BK_GEOM_PACK);  /* Tk's packer */

  BKContext *ctx = BK_context_new(root);
  BKHost host = {0};                        /* the window fills it in */
  BK_context_set_host(ctx, &host);
  BKWindow *win = BK_window_new(ctx, "BUI from C", 320, 120);
  BK_window_show(win);
  BK_window_run();
  return 0;
}`,
  install: "cmake -S . -B build && cmake --build build",
  shot: "c-kit-app",
  fallback: "images/shots/c-kit-app-dark.png",
  alt: "A modifier stack from the C kit: panels, enum rows, number fields, dropdowns, a slider, checkboxes and buttons",
  caption: "examples/site/kit_modifiers.c, the C kit",
  docs: "docs/c/c-api/",
  source: "docs/c-api.md",
};

const lua: Syntax = {
  id: "lua",
  label: "Lua",
  icon: "lua",
  title: "Every widget, from Lua",
  blurb: "A bui-lua executable and a bui module for Lua 5.4, LuaJIT and LuaRocks. Nodes, properties and events, the same as the C kit.",
  lang: "lua",
  file: "counter.lua",
  code: `local bui = require("bui")

local root = bui.Node("frame")
root:set("padding", 12)

local label = bui.Node("label")
label:set("text", "Count: 0")
root:add(label)
label:geom(bui.GEOM_PACK, {side = 0, fill = 1})

local count = 0
local inc = bui.Node("button")
inc:set("text", "Increment")
inc:on(bui.EV_CLICK, function()
    count = count + 1
    label:set("text", "Count: " .. count)
end)
root:add(inc)
inc:geom(bui.GEOM_PACK, {fill = 1, pady0 = 4})

local win = bui.Window(bui.Context(root), "Counter", 240, 200)
win:show()
bui.run()`,
  install: "luarocks install blenderui",
  shot: "lua-editor",
  fallback: "images/shots/lua-editor-dark.png",
  alt: "A table editor written in Lua: a tree view with three columns, a toolbar with search and undo, and a collapsible editor panel",
  caption: "bindings/lua/examples/editor_screenshot.lua, the table editor",
  docs: "docs/lua/overview/",
  source: "bindings/lua/examples/counter.lua",
};

const go: Syntax = {
  id: "go",
  label: "Go",
  icon: "go",
  title: "MyGo-style views",
  blurb: "The view builds the window from your state and runs again after input. Controls bind to the state, and a headless Tester checks it.",
  lang: "go",
  file: "main.go",
  code: `type counter struct{ n int }

func (s *counter) view(c *ui.Context) {
	if c.Shortcut(0, ui.KeyUp) {
		s.n++
	}
	ui.Column(c).Fill().Center().Gap(16).Children(func() {
		ui.Textf(c, "%d", s.n).FontSize(56).Bold()
		ui.Row(c).Gap(8).Children(func() {
			if ui.Button(c.Key("decrement"), "−").Width(44).Clicked() {
				s.n--
			}
			if ui.Button(c.Key("reset"), "Reset").Disabled(s.n == 0).Clicked() {
				s.n = 0
			}
			if ui.PrimaryButton(c.Key("increment"), "+").Width(44).Clicked() {
				s.n++
			}
		})
	})
}`,
  install: "go run ./examples/counter",
  shot: "go-counter",
  fallback: "images/shots/go-counter-dark.png",
  alt: "A counter written in Go: a large number, decrement and increment buttons and a Reset button",
  caption: "bindings/go/examples/counter, MyGo style",
  docs: "docs/go/mygo-syntax/",
  source: "bindings/go/examples/counter/main.go",
};

const ruby: Syntax = {
  id: "ruby",
  label: "Ruby",
  icon: "ruby",
  title: "Interfaces built with blocks",
  blurb: "A C extension gem, no FFI. Columns, rows and buttons are blocks, and the same interface renders without a window for tests.",
  lang: "ruby",
  file: "counter.rb",
  code: `require "bui"

class Counter
  attr_reader :count

  def initialize(ui)
    @count = 0
    ui.column padding: 16, spacing: 8 do |col|
      @shown = col.label "0", text_align: "center", bold: true
      col.row spacing: 4 do |row|
        row.button("Decrement") { self.count = count - 1 }
        row.button("Reset") { self.count = 0 }
        row.button("Increment") { self.count = count + 1 }
      end
    end
  end

  def count=(n)
    @count = n
    @shown.text = n.to_s
  end
end

BUI.app do
  win = window("Counter", width: 300, height: 120)
  Counter.new(BUI::Builder.new(win.root))
end`,
  install: "bundle install && ruby -Ilib examples/counter.rb",
  shot: "ruby-counter",
  fallback: "images/shots/ruby-counter-dark.png",
  alt: "A counter written in Ruby: the count, and Decrement, Reset and Increment buttons",
  caption: "bindings/ruby/examples/counter.rb",
  docs: "docs/ruby/overview/",
  source: "bindings/ruby/examples/counter.rb",
};

export const languages: Language[] = [
  { id: "python", tab: "Python", icon: "python", syntaxes: [tkinter, qt] },
  { id: "rust", tab: "Rust", icon: "rust", syntaxes: [rsx, gpui, htmlBuilder] },
  { id: "javascript", tab: "JavaScript", icon: "javascript", syntaxes: [react, svelte, vue, dom] },
  { id: "imgui", tab: "Dear ImGui", icon: "cpp", syntaxes: [imguiCpp, imguiC, imguiPy, imguiRust] },
  { id: "c", tab: "C", icon: "c", syntaxes: [c] },
  { id: "lua", tab: "Lua", icon: "lua", syntaxes: [lua] },
  { id: "go", tab: "Go", icon: "go", syntaxes: [go] },
  { id: "ruby", tab: "Ruby", icon: "ruby", syntaxes: [ruby] },
];
