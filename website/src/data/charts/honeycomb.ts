import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A hexagon per thing, coloured by how it is doing, packed to fill the panel.",
  intro: [
    "A rack of servers is a surprisingly hard thing to draw. A bar chart of 120 hosts is a comb with no teeth. A table of 120 rows is a table of 120 rows. What you want to know is almost always a single question about the whole fleet: how many are fine, and which are not?",
    "The honeycomb answers it the way a bee would. Each host gets a cell, the cell takes a colour from your thresholds, and the cells are packed so that the whole panel is filled. There is no axis to read. There is a mostly green field with a few cells that are not, and your eye lands on them in about a quarter of a second. It is the same idea as a status grid, with the practical advantage that hexagons tile without leaving the slightly sad gaps that squares do at the edges.",
    "Bees settled on hexagons because they use [the least wax](https://en.wikipedia.org/wiki/Honeycomb_conjecture) for the most storage. You will have a different reason: they look better on a wall monitor, and nobody has ever been asked to give up a hexagon.",
  ],
  math: [
    {
      heading: "Packing hexagons into a rectangle",
      body: [
        "Suppose you have $n$ cells, a drawing width $W$ and a height $H$ (the panel minus an 8 px margin). With $c$ cells per row there are $r = \\lceil n / c \\rceil$ rows. Odd rows are shifted half a cell to the right, so with more than one row the layout is $c + \\tfrac12$ cell-widths wide. For a hexagon of circumradius $R$, the cell width is $\\sqrt{3}\\,R$ and each row sits $1.5\\,R$ below the last.",
        "The largest radius that fits is limited by whichever of the two dimensions runs out first. The chart tries every $c$ from 1 to $n$ and keeps the one with the largest $R$, which is why the cells get bigger and the grid shape changes as you resize the panel. You can force $c$ with `columns`.",
      ],
      tex: [
        "R(c) = \\min\\!\\left(\\frac{W}{\\sqrt{3}\\,(c + \\tfrac12)},\\; \\frac{H}{1.5\\,r + 0.5}\\right), \\qquad r = \\lceil n/c \\rceil",
      ],
    },
    {
      heading: "Colour",
      body: [
        "Each cell's colour comes from its value $v$ and the threshold steps. A step is the last one whose bound is not above $v$. Percentage thresholds are turned into absolute values using `min` and `max` (0 and 100 by default), so \"80%\" means $a + 0.8\\,(b - a)$ for a range $[a, b]$.",
        "A missing or non-finite value is not coloured as a zero: the cell is drawn in the theme's neutral grey at 40% opacity. That difference matters, because a host that has stopped reporting is not a host with a CPU of 0.",
      ],
      tex: [
        "\\text{threshold}(p\\%) = a + \\frac{p}{100}\\,(b - a)",
      ],
    },
    {
      heading: "Text and drawing",
      body: [
        "The hexagon is drawn at 96% of the cell radius with a 1 px outline in the background colour, which is what produces the thin gaps between cells. The value and the name are written inside only when the cell is large enough ($R > 22$ px and the text narrower than about 90% of the cell width). The value font is $0.34\\,R$ clamped between 9 and 26 px, and long names are cut and ended with an ellipsis. Hovering a cell shows a tooltip with its name and value.",
      ],
    },
  ],
  use: [
    "You have tens to a few hundred similar things (hosts, pods, sensors, stores) and want the state of the whole set at a glance.",
    "The question is \"which ones are out of range?\", not \"what is the exact value?\".",
    "You have natural thresholds, such as good under 80% and bad above, and a colour per step is enough resolution.",
    "You need the layout to adapt to whatever size the panel is: the grid shape recomputes itself.",
  ],
  avoid: [
    "You need to compare values precisely. Colour is a poor measuring tool. Use a [Bar Gauge](/charts/bar-gauge) or a [Lollipop](/charts/lollipop).",
    "The order of items carries meaning, such as time. Cells are laid out in data order, row by row, so position says nothing on its own. Use a [Status History](/charts/status-history) for samples over time.",
    "You have thousands of cells. At that point each one is a few pixels, with no labels, and you are making a heatmap whether you meant to or not.",
    "You want to see a distribution. A [Histogram](/charts/histogram) shows it directly.",
  ],
  read: [
    "Look at the colour field first. A mostly uniform panel is good news. The question is which cells stand out.",
    "Find the outliers. Order is data order, so sort your data (by region, by rack, by value) if you want the outliers to cluster.",
    "Read the labels where they fit: the value on top and the name below. Where they do not fit, the cell is too small, and you can hover it instead.",
    "Grey cells have no value. Treat them as a separate category from green ones.",
  ],
  story: {
    title: "Two hundred and eleven smoke detectors",
    paragraphs: [
      "Rosalind managed facilities for a chain of cold-storage warehouses, which meant a lot of sensors and a small team, and a standing rule that if a freezer door was left open, the cost was measured in prawns. Each of the 211 freezer units reported its temperature every minute to a central collector.",
      "The old report was a spreadsheet exported every morning, with conditional formatting that had been painted red by somebody who clearly believed in the colour. It was a long column of numbers, and the units that mattered were hidden in the middle of it.",
      "What she needed was a single view that a shift supervisor could glance at on the way to the loading dock: the whole estate, one mark per unit, and an immediate reading on which were warm. Order did not matter much. Presence of a problem did.",
      "She built it with bui-charts: `<HoneyComb>` with the unit name as the label, the temperature as the value, and thresholds at -18 and -15 degrees, so anything warmer than -15 went red. She set `unit=\"°C\"`, left `columns` unset so the layout fitted the wall monitor, and sorted the data by site so that each warehouse formed its own patch.",
      "On the second night, a patch of three cells in warehouse four turned amber, then red. It was a defrost cycle that had stuck. The supervisor saw it from across the dock, walked over and reset it. The temperature in those units had reached -14.2.",
      "The spreadsheet had shown the same three rows the morning after, a day late. Over the next quarter the number of units that spent more than an hour above -15 fell from nineteen to four, and a small number of prawns, who have no say in this, were spared.",
      "The old spreadsheet is still emailed every morning at six. Nobody has been able to find who set it up.",
    ],
  },
  pitfalls: [
    "**Data order is layout order.** Cells fill left to right, row by row. If your data is in random order, so is your panel, and patterns will not show.",
    "**Offset rows are not columns.** Odd rows are shifted by half a cell, so the cell below is not directly under the one above. Do not read vertical alignment as meaning.",
    "**Resizing reflows the grid.** Because the number of columns is picked to maximise cell size, the same data can be 12 across in a wide panel and 6 across in a narrow one.",
    "**Threshold direction.** The defaults are good below 80 and bad above. For free space or battery, where low is bad, build the thresholds the other way round.",
    "**Tiny cells lose their text.** With many cells in a small panel, labels disappear below about 22 px radius. That is the layout being honest. Either enlarge the panel or split it by group.",
  ],
  related: [
    { slug: "status-history", why: "When you need the state of each item over time, not just now." },
    { slug: "waffle", why: "When you need to count units that make up a share, not diagnose individual items." },
    { slug: "bar-gauge", why: "When values need to be compared by length rather than colour." },
  ],
  media: [
    {kind:"youtube",id:"8OmBkDzCvjI",title:"How to Make a Hexagonal Cartogram in ArcGIS Pro",channel:"John Nelson Maps",note:"About hexagonal cartograms made with ArcGIS Pro, a neighbouring tool and technique that shares the hex-cell layout."},
  ],
}
