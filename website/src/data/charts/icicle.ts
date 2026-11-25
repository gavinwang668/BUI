import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A tree drawn as stacked rows of blocks, root at the top, each block as wide as the value it holds.",
  intro: [
    "Somewhere on your disk there is a folder called `old_stuff_final_v2`, and it is eating forty gigabytes. You know this because a tool told you, in a list sorted by size, and you have not opened it since. Lists are good at telling you which single thing is biggest. They are bad at telling you *where in the tree* the weight lives, and that is almost always the real question.",
    "An icicle chart answers it with geometry. The whole is one wide block at the top. Beneath it, each child is a block sitting right under its parent, with a width proportional to its share. Beneath those, the grandchildren, and so on, until the tree runs out. Everything is aligned, so a block's width can be compared with every other block on the same row, and a heavy branch is simply a column of tall blocks that keeps going down. The name is because it looks like icicles hanging from a gutter. Kruskal and Landwehr described icicle plots in the early 1980s for showing hierarchical clusterings, and the form has since escaped into disk analysers and profilers.",
    "It is a [flame graph](https://www.brendangregg.com/flamegraphs.html) turned upside down: same blocks, same widths, root at the top because that is where people expect the start of a tree to be. In bui-charts it literally is the flame graph pane, so everything that works there (folded stacks, nested sets, zoom on click) works here.",
  ],
  math: [
    {
      heading: "Totals and self time",
      body: [
        "A profile or any tree with sizes is a set of paths. Each leaf path carries a number, such as a sample count or a byte size, and an internal node's **total** is everything beneath it, including the part it contributes itself. Write $T(v)$ for the total of node $v$ and $S(v)$ for its *self* value, the part not accounted for by any child.",
        "Input in folded form (lines like `main;parse;lex 12`) is added up per path, so repeated stacks merge. The root's total is the sum of everything, and that is the width of the whole chart.",
      ],
      tex: [
        "T(v) = S(v) + \\sum_{c \\,\\in\\, \\text{children}(v)} T(c)",
      ],
    },
    {
      heading: "Width is share, depth is row",
      body: [
        "Let the chart be $W$ pixels wide. A block for node $v$ starts where its previous sibling ends, inside its parent's span, and is as wide as its share of the root. Siblings never overlap, and a node's children together are never wider than the node, with any difference being the node's self value, drawn as the empty strip at the right end of the row beneath it.",
        "Depth is the row: the root is row $0$ at the top and a node at depth $d$ is drawn $d$ rows down. Because all rows are the same height, area is **not** the encoding here. Only width is. Compare blocks left to right within a row, and read depth as a count of levels.",
      ],
      tex: [
        "w(v) = W \\cdot \\frac{T(v)}{T(\\text{root})}, \\qquad y(v) = d(v) \\cdot h",
      ],
    },
    {
      heading: "Zooming",
      body: [
        "Clicking a block makes it the new root of the view: its total becomes the full width and its children are redrawn at the new scale, so a block that was a sliver can fill the screen. The same rule applies after the zoom: width is the block's share of whatever is now the top. Very narrow blocks may be too thin to draw a label inside, and zooming is how you read them.",
      ],
    },
  ],
  use: [
    "You have a tree with **sizes** and the question is *where is the weight*: disk usage by folder, memory by allocator, a budget by department and line item.",
    "The tree is deep or irregular. A table of folders goes eight levels down and nobody ever scrolls to the bottom.",
    "You have a CPU or memory profile and want the root-on-top orientation that most readers expect from a tree.",
    "You want to compare a block against its siblings *and* against its cousins. Everything on one row shares one scale, so width comparisons work across branches.",
  ],
  avoid: [
    "You need the table, the sandwich view or a before-and-after diff. That is what [`<FlameGraph>`](/charts/flame-graph) adds on top. Use it when you want to hunt for a hot function.",
    "The tree is shallow and has a few dozen leaves. A [treemap](/charts/treemap-force) or a bar chart is more compact and easier to label.",
    "There are no sizes, only structure. An icicle with equal weights is a bad tree diagram.",
    "You want exact comparisons between non-adjacent blocks. Widths are easy to compare on one row and surprisingly hard between far-apart rows.",
  ],
  read: [
    "Start at the top: the root is the whole. Its width is everything you have.",
    "Look down, one row at a time. A wide block directly under the root is a branch that holds a large share of the whole.",
    "Follow a heavy block downward. If it stays wide for many rows, the weight really sits deep in that branch, if it splits into slivers, it is spread across many small things.",
    "Click a block to make it the new top and read its children at full width. Reset to go back up.",
    "Hover for the exact value and the share, since labels get squeezed out in thin blocks.",
  ],
  story: {
    title: "The folder that was not on the list",
    paragraphs: [
      "Gareth looked after the file server for a small architecture practice, which is a polite way to say he looked after a 4 TB drive that everyone treated as a public bin. It was at 94% and the last time anyone had cleaned it, the intern responsible no longer worked there.",
      "He knew how to find large files. What he did not know was where to start deleting. The partners would say yes to removing archived projects and no to removing live ones, and the drive had 140,000 files across 600 folders with names like `Final`, `Final_FINAL` and `Final_use_this`.",
      "A sorted list of the biggest files was no use, because the biggest file was a 38 GB render nobody dared touch. He needed to see which *branches* were heavy, in a way the partners could look at and say yes or no to, ideally without opening Explorer in a meeting.",
      "He exported the folder sizes as folded paths, one line per folder with its size in gigabytes, and passed them to `<Icicle>` in bui-charts as a profile. The tree appeared with the drive at the top and the projects hanging beneath it like icicles.",
      "One column dominated. A project folder from 2017 was 410 GB, and under it a subfolder called `Renders` was 390 of them, and under *that* were nine copies of the same scene at slightly different resolutions. It was the thickest, darkest icicle on the chart, and no partner needed an explanation.",
      "The partners approved deleting seven folders in five minutes, because the chart made it obvious what each one would free. Gareth got back 1.1 TB, the drive went to 61%, and the nine renders were reduced to two. The 2017 project was, as far as anyone could tell, a restaurant. It has not been missed.",
      "He now runs the same chart every quarter, mostly so that nobody can say they were not warned.",
    ],
  },
  pitfalls: [
    "**Width is the only encoding.** Height is just the level, so a tall column means a deep tree, not a heavy one. Judge weight by width.",
    "**Thin blocks hide their labels.** That is the chart being honest: small things get small blocks. Click to zoom, or hover, before concluding something is missing.",
    "**Self value is not drawn as a block.** If a node's children add up to less than its own total, the gap shows as empty space under it, that remainder is its self value.",
    "**Pass a profile, not a rebuilt one every frame.** Build the data once and keep it in a signal, otherwise it is processed again on every render.",
    "**Header is off by default.** Turn on `header` if you want search and reset. Leave it off when the icicle is part of a bigger dashboard.",
  ],
  related: [
    { slug: "flame-graph", why: "Same blocks, with the table, sandwich view and diffs for when you are hunting a specific function." },
    { slug: "sunburst", why: "The same kind of tree wrapped into rings. Better for a few levels and a roughly even spread." },
    { slug: "treemap-force", why: "When you want sizes as areas and do not need to see depth as rows." },
  ],
  media: [
    {kind:"youtube",id:"VaqTVzXNobQ",title:"Maps Tree Maps & Sunburst Diagrams in Excel [HBDXL]",channel:"Ann K. Emery",note:"Covers treemaps and sunburst charts, neighbouring hierarchical charts, not icicle charts specifically."},
  ],
}
