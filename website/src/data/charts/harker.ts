import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A grid of scatter plots, one per oxide, each against silica, for reading how a rock suite evolved.",
  intro: [
    "Igneous rocks from one volcano, or one batholith, are usually cousins. They share a parent magma, and the question that makes geologists lean over a table is how that parent turned into the many children in the drawer. The trick for telling is to pick one oxide as a ruler, silica, and plot every other oxide against it. If the rocks are related, the points do not scatter. They line up.",
    "[Alfred Harker](https://en.wikipedia.org/wiki/Alfred_Harker), a Cambridge petrologist, popularised this kind of [variation diagram](https://en.wikipedia.org/wiki/Harker_diagram) in his 1909 book on the natural history of igneous rocks, and geochemists have been drawing them ever since. A grid of small plots, one for magnesium, one for calcium, one for potassium, is still the first figure in half the geochemistry papers published, which is roughly the same fraction that then spend a paragraph explaining why one panel does not quite behave.",
    "The chart is the grid. You give it the rows and the silica value, add one child per oxide, and it lays out the panels on a shared silica axis. Because the panels are separate plots, each oxide gets a y axis that fits its own range, and a trace element that varies by a few parts per million is not flattened by a major oxide that varies by ten percent.",
  ],
  math: [
    {
      heading: "One ruler, many panels",
      body: [
        "Let row $k$ be one rock, with silica $s_k$ in weight percent and oxides $y_{k,1}, \\dots, y_{k,m}$ declared with `<Oxide name=\"...\" value=...>` children. Panel $j$ plots the points below. The x axis is labelled SiO₂ wt%, the y axis is labelled with the oxide name and wt%, and each panel chooses its own y range from its data.",
      ],
      tex: [
        "\\bigl(s_k,\\; y_{k,j}\\bigr), \\qquad k = 1,\\dots,N,\\quad j = 1,\\dots,m",
      ],
    },
    {
      heading: "Layout",
      body: [
        "The oxides are laid out in rows of `columns` (3 by default, never fewer than 1). So $m$ oxides fill $\\lceil m / c \\rceil$ rows, with each panel sharing the available width equally; if the last row is short, empty cells are added so the panels keep the same size. All panels use one colour and one marker size (5 px by default). The panels have no legend and no tooltip, since the label is on the axis and the grid is for pattern, not lookup.",
      ],
    },
    {
      heading: "Why straight lines mean something",
      body: [
        "If two end-member magmas $A$ and $B$ mix in a fraction $f$, every oxide mixes linearly with the same $f$, so the hybrid rock lies on a straight line between the end members in every panel at once. That is the signature of magma mixing. [Fractional crystallisation](https://en.wikipedia.org/wiki/Fractional_crystallization_(geology)), in which minerals are removed from a cooling magma, leaves *curved* trends instead, and they bend where a new mineral begins to crystallise: the kink in a panel is the evidence for what came out of the melt.",
      ],
      tex: [
        "y_{\\text{mix}} = f\\,y_A + (1-f)\\,y_B, \\qquad s_{\\text{mix}} = f\\,s_A + (1-f)\\,s_B",
      ],
    },
  ],
  use: [
    "You have major-element analyses of a related rock suite and want to see whether the variation looks like mixing, crystal removal or something else.",
    "You are screening data before a deeper model: a panel with a scatter that should be a line is a quick flag for a bad analysis or a different rock.",
    "You want to compare how several oxides respond to the same driver in one figure, with a fitted y range for each.",
    "You want the first figure of a geochemistry paper that a reviewer will recognise at a glance.",
  ],
  avoid: [
    "The samples are not from one suite. Plotting unrelated rocks against silica produces a cloud, and a cloud can be read as anything.",
    "You want to classify the rock. A total-alkali against silica diagram is the standard tool for names. This chart is for trends.",
    "You want to compare oxides on one set of axes. The panels have different y ranges on purpose.",
    "You have more than about a dozen oxides. The panels shrink until the trends are unreadable. Choose the six or eight that tell the story.",
  ],
  read: [
    "Check the silica axis first. All panels share it, so a gap in silica appears as a gap in every panel.",
    "Look for the overall slope in each panel: oxides that fall as silica rises (magnesium, iron, calcium) and ones that rise (potassium, sodium) are the textbook pattern.",
    "Look for bends. A change of slope is usually a mineral joining or leaving the crystallising assemblage.",
    "Look for outliers: one point off the trend in one panel only is more often an analytical error or an altered sample than a new magma.",
  ],
  story: {
    title: "Two magmas and a stubborn bump",
    paragraphs: [
      "Kofi was a field geologist mapping a volcanic centre in Ghana's Volta region, where the rocks were old, the roads were optimistic and the budget covered exactly one analytical run. He came back with 64 samples ranging from dark basalt to pale rhyolite, and a supervisor who wanted to know whether this was one magma getting fancy or two magmas getting acquainted.",
      "The lab sent back a spreadsheet: silica, titanium, aluminium, iron, magnesium, calcium, sodium and potassium, in weight percent, to two decimal places and no explanation. Plotted one at a time in a general chart, the oxides were seven separate pictures and no argument.",
      "What he needed was the classic: all oxides against silica, side by side, each on a scale that suited it, so that the same rocks could be compared across panels. He also needed to see the whole thing at once in one pane, because his argument depended on a feature in one panel matching a feature in another.",
      "He gave `<Harker>` the rows, the silica accessor and seven `<Oxide>` children, with `columns={4}` for two tidy rows. The grid opened and the answer arrived in the first panel: the magnesium trend fell in two straight segments, one steep and one shallow, meeting at a kink near 55 percent silica.",
      "Straight segments are not what crystallisation does. Curves are. Two clean lines with a hinge suggested a mixing line from a basalt end member to something intermediate, then a second, from there to the rhyolite. The titanium panel showed the same break at the same silica. So did all the others, each in its own way, which is hard to do by coincidence across seven oxides.",
      "That became the paper: two mixing events, not one long fractionation, and a basaltic magma that had recharged a chamber half-full of older evolved liquid. The estimate of the recharge fraction, 28 percent, came from reading the position of the hinge against the end members.",
      "Kofi's supervisor read it twice, said it was the only paper he had ever finished that contained a good argument and a sensible number of figures, and asked for an eighth panel. There was no eighth oxide, and the seven panels stayed.",
    ],
  },
  pitfalls: [
    "**Wrong units.** The axes are labelled in weight percent. Trace elements in ppm will have y values in the hundreds under a percent label. Convert or relabel before presenting.",
    "**Normalise first.** Analyses are usually recalculated to 100 percent on a volatile-free basis before plotting. Raw totals that vary from 96 to 101 add scatter that is not geology.",
    "**One colour only.** All points share one colour, so you cannot tell different units or ages apart. If groups matter, plot them separately with the `color` prop and compare the figures.",
    "**Oxides with different scales.** Panels fit their own y ranges, so two panels with a similar slope can represent very different amounts of change. Read the tick labels.",
  ],
  related: [
    { slug: "ternary", why: "When three components sum to a whole and you want to see all of them at once, as in a mineral or rock classification." },
    { slug: "piper", why: "When the data are water chemistry and you want to see the mix of major ions in a sample." },
    { slug: "xy-chart", why: "When you want a single panel with full control of axes, series and tooltips." },
  ],
  media: [
    { kind: "youtube", id: "Zt0G16AHL9g", title: "Harker Diagram- Igneous Petrology | GATE/NET IIT-JAM | GeologyConcepts.com", channel: "Geology Concepts", note: "Explains what Harker variation diagrams show in igneous petrology." },
    { kind: "youtube", id: "UAyBpsIEN8w", title: "Igneous Petrology Lesson 1 Continue: Variation Diagrams (Harker, AFM & QAPF)", channel: "MJ SCHOOL OF MINING AND GEOLOGY", note: "Lesson placing Harker diagrams alongside other variation diagrams." },
  ],
}
