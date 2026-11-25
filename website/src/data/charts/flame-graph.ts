import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A profile drawn as stacked boxes: width is time spent, height is call depth, and the wide ones are your problem.",
  intro: [
    "A profiler produces a lot of stack traces. Thousands per second, each one a list of functions that were running at that instant, and a text file of them is roughly as readable as the phone book of a city you have never visited. The question you actually have is short: *where is the time going?*",
    "[Brendan Gregg](https://www.brendangregg.com/flamegraphs.html) invented the flame graph around 2011 while chasing a performance problem at Joyent, and the idea is almost embarrassingly simple. Merge the stacks that share a prefix. Draw the root at the bottom, each callee on top of its caller, and make every box as wide as the number of samples it appears in. Hot code becomes wide. Deep code becomes tall. Code that does nothing in particular becomes invisible, which is the best thing that can happen to code.",
    "The name comes from the shape, a skyline with flickering tops, and does not mean what you hope it means. A wide flame is not a fire you have to put out. It is somewhere you could look. Whether it is a bug or just the part of the program that does the work is a judgement the chart cannot make for you, and the most common mistake is to optimise the widest box in the graph, which is usually called `main`.",
  ],
  math: [
    {
      heading: "Layout from samples",
      body: [
        "A profile is a list of nodes in depth-first order, each with a level and a value (the number of samples, nanoseconds or bytes spent in that frame *including* its callees). The chart lays them out left to right: a node starts where its previous sibling ends, or at its parent's start if it is the first child. With $T$ the value of the root and $v_n$, $a_n$ the value and start of node $n$:",
        "The width of a node is therefore its share of the whole profile, and widths of children never exceed the width of their parent. The remainder of a parent that is not covered by children is its **self** time, the part of the box that has nothing above it.",
      ],
      tex: [
        "a_n = \\begin{cases} a_{\\text{parent}} & \\text{first child} \\\\ a_{\\text{prev}} + v_{\\text{prev}} & \\text{otherwise} \\end{cases} \\qquad \\text{self}_n = v_n - \\sum_{c \\in \\text{children}(n)} v_c",
      ],
    },
    {
      heading: "From share to pixels",
      body: [
        "If the view shows the fraction $[z_0, z_1]$ of the profile (full width until you zoom) in $W$ pixels, one unit of value is worth $W / \\bigl(T\\,(z_1 - z_0)\\bigr)$ pixels, and a box is drawn $v_n$ times that wide. Boxes under half a pixel are not drawn, boxes of 10 pixels or less lose their border and are muted, and a label only appears when the box is at least 20 pixels wide.",
        "A chain of single calls is a common source of tall, thin skinny towers. By default, a node that is the only child of its parent and carries more than 99% of the parent's value is grouped with it onto one row, so the graph shows the call that matters and not the six wrappers around it. `collapsing={false}` shows every frame.",
      ],
      tex: [
        "w_n = v_n \\cdot \\frac{W}{T\\,(z_1 - z_0)}",
      ],
    },
    {
      heading: "Differential profiles",
      body: [
        "`Profile::diff(&before, &after)` merges two profiles on the same call paths. Because the two runs rarely have the same length, the chart compares *shares*, not raw values: for a frame with values $l$ and $r$ in profiles of total $L$ and $R$, it takes percentages rounded to two decimals, $p_l = 100\\,l/L$ and $p_r = 100\\,r/R$, and computes the relative change, clamped to $\\pm 100$.",
        "Zero change gets the neutral colour, and the colour mixes towards the \"better\" end for negative change and towards the \"worse\" end for positive change in proportion to $|\\Delta|/100$. A frame that exists only after has an infinite relative change and is clamped to full intensity. If either total is zero the neutral colour is used.",
      ],
      tex: [
        "\\Delta = \\mathrm{clamp}\\!\\left(100\\,\\frac{p_r - p_l}{p_l},\\ -100,\\ 100\\right)",
      ],
    },
  ],
  use: [
    "You need to find where a program spends **CPU time, wall time, memory or allocations**, and you have stack samples.",
    "You want to compare two builds. A diff profile colours what got relatively slower or faster.",
    "The profile is huge. Hundreds of thousands of samples reduce to a few dozen visible boxes, and you can zoom into any of them.",
    "A flat top table is not enough: you need to know *who called* the expensive function and what it called.",
  ],
  avoid: [
    "You have measurements over time rather than stack samples. A flame graph merges away the time axis. Use [Trend](/charts/trend) or [Time Series](/charts/time-series).",
    "The hierarchy is not a call tree but a set of categories with sizes. An [icicle](/charts/icicle) or [sunburst](/charts/sunburst) is a closer fit.",
    "You want exact counts for a short list of functions. The top table pane does that better, and is built in.",
    "The profile has very few samples. A flame graph of 40 samples is a drawing of noise.",
  ],
  read: [
    "Read **width** first. It is the share of the profile that includes this function and its callees. Height is depth, not cost.",
    "Look for wide boxes with *nothing on top of them*. That is self time, the work done in that function itself, and usually the real hot spot.",
    "Follow a wide tower down from `main` to find the path that leads to it. Colour groups related code by package by default, so you can see which library the time is in.",
    "Click a frame to focus on it and zoom in, and use the menu for the sandwich view, which shows all callers and callees of a function across the whole profile. Search to highlight matches.",
  ],
  story: {
    title: "Why the nightly export took four hours",
    paragraphs: [
      "Lucia maintained the invoice export at a logistics software company in Rotterdam, a Rust service that every night turned a million shipments into a pile of PDFs and a spreadsheet nobody read. For two years the job finished in about forty minutes. In March it started taking four hours, and the finance team started describing it, at length, as *the slow thing*.",
      "Her first guess was the database, because it is always the first guess. Her second guess was the new PDF library, because it was the only thing that had changed. She had reasons to believe both, and no way of choosing between them without a lot of arguing.",
      "She ran the job under `perf` for ten minutes on a copy of the production data and got a file of folded stacks, one line per distinct call path, with a count at the end. It was 61,000 lines long. She did not read it.",
      "She loaded it into bui-charts with `Profile::from_folded` and showed it in a `<FlameGraph>`, then did the same for last year's build and asked for a `Profile::diff`. The diff coloured every frame by how much its share of the run had changed, and nearly the whole graph came out a pale, uninteresting grey.",
      "One tower came out red. It sat under a function called `normalize_address`, and it was wide: 71 per cent of the profile, where a year ago it had been 4. The database was fine. The PDF library was fine. A well-meaning change had added a regular expression, compiled inside the loop, to every one of a million addresses.",
      "Moving the compile out of the loop took one line. The job went back to thirty-eight minutes, finance stopped talking about the slow thing, and the commit message, per tradition, said *fix perf*.",
      "Lucia keeps the red tower as her desktop background. The regular expression, she maintains, was always going to be the problem. Nobody in the room was able to say the same about the database, so everybody was forgiven.",
    ],
  },
  pitfalls: [
    "**Width is not self time.** A wide box can be wide only because its callees are. Check what sits on top of it.",
    "**Sampling is statistical.** Very short boxes are noisy and a function that runs for 1 ms once may never appear. Capture long enough for the question you are asking.",
    "**Left to right is not time.** Siblings are placed in sample order or alphabetically, depending on the profiler, so the x axis is not a timeline. Do not read a sequence into it.",
    "**Inlined and missing frames.** Optimised builds inline functions away and omit frame pointers, and the graph then shows what the profiler could recover, not what you wrote.",
    "**Passing a `Profile` directly.** It is processed again on every render. Build it once and keep it in a signal.",
  ],
  related: [
    { slug: "icicle", why: "The same idea hanging downward, for hierarchies that are not call stacks." },
    { slug: "sunburst", why: "A hierarchy of sizes on rings, when depth is shallow and you want a compact picture." },
    { slug: "traces", why: "When you have individual timed requests and need the timeline, not the merged stacks." },
  ],
  media: [
    {
      kind: "youtube",
      id: "nZfNehCzGdw",
      title: "LISA13 - Blazing Performance with Flame Graphs",
      channel: "USENIX",
      note: "The inventor's own plenary talk on what flame graphs show, how they are built, and how to read them.",
    },
    {
      kind: "youtube",
      id: "D53T1Ejig1Q",
      title: "USENIX ATC '17: Visualizing Performance with Flame Graphs",
      channel: "USENIX",
      note: "A tighter, later talk covering the colour conventions, the differential flame graph and common pitfalls.",
    },
  ],
}
