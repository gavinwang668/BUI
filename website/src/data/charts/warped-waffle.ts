import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A waffle chart bent into a ring: a fixed number of cells, shared out by value, filled around the circle.",
  intro: [
    "A waffle chart is a grid of 100 squares in which 37 are one colour. It is the pie chart for people who have been told that pie charts are bad, and it has one real virtue: you can count. Nobody has ever had to estimate an angle to find out that 37 squares are 37 per cent.",
    "A warped waffle takes the same idea and wraps the grid around a circle. The rows become rings, the columns become sectors, and the whole thing ends up looking like a radar screen or a dartboard that has been taught about percentages. It is a good deal more decorative than a square grid, and you should say so out loud before anyone else does.",
    "That makes it a chart for dashboards, report covers and the middle of a slide where the number in the hole is the headline. It is a poor choice for a technical appendix. But as a \"here is the split, and here is the total\" graphic it is hard to beat, because the cells are still countable and the middle is empty enough to carry the figure that matters.",
  ],
  math: [
    {
      heading: "Sharing out a fixed number of cells",
      body: [
        "The chart has $N = R \\times S$ cells, with $R$ rings (default 5) and $S$ sectors (default 20), so 100 by default. Each category with value $v_i$ is entitled to an exact share of the cells. Negative and non-finite values count as zero. Hidden categories (clicked off in the legend) are also set to zero before sharing, so the remaining ones refill the whole ring.",
        "Shares are rarely whole numbers, so the cells are handed out with the **[largest remainder method](https://en.wikipedia.org/wiki/Largest_remainders_method)**: every category first gets the floor of its entitlement, then the leftover cells go, one each, to the categories with the biggest fractional parts. Categories with no value never receive one. This guarantees the counts add to exactly $N$.",
      ],
      tex: [
        "e_i = \\frac{v_i}{\\sum_j v_j}\\,N, \\qquad n_i = \\lfloor e_i \\rfloor + [\\,i \\text{ among the largest fractional parts}\\,]",
      ],
    },
    {
      heading: "From counts to the ring",
      body: [
        "Cells are laid out sector by sector, and within a sector from the inner ring outward: cell number $k$ belongs to sector $\\lfloor k / R \\rfloor$ and ring $k \\bmod R$. The first category therefore fills whole radial slices and the next starts where it stopped, so each category is a contiguous run around the circle with at most two ragged ends.",
        "With outer radius $\\rho$ (the shorter side of the chart over two, less 8 px), hole fraction $h$ (default 0.35, clamped to 0 to 0.9) the inner radius is $h\\rho$, and each ring has equal depth $\\delta$. Every sector spans $2\\pi / S$. A gap fraction (default 0.12) shrinks each cell on every side so the cells read as separate tiles.",
      ],
      tex: [
        "\\delta = \\frac{\\rho(1-h)}{R}, \\qquad r_k = h\\rho + k\\,\\delta, \\qquad \\theta = \\frac{2\\pi}{S}",
      ],
    },
    {
      heading: "What the warp does to area",
      body: [
        "Ring $k$ has the same depth as every other, so its cells get larger the farther out you go. Each cell's area is below. The counts are exact, but the areas are not equal: an outer cell can be several times the size of an inner one when the hole is small.",
        "This is the price of the circle. For counting it is harmless, because you count cells and ignore their size. For comparing by area it is a distortion, so do not put two warped waffles side by side and eyeball them. The text in the middle shows the total (or the word \"shown\" if some categories are hidden).",
      ],
      tex: ["A_k = \\tfrac{\\theta}{2}\\,\\bigl(r_{k+1}^2 - r_k^2\\bigr) = \\tfrac{\\theta}{2}\\,\\delta\\,(2 r_k + \\delta)"],
    },
  ],
  use: [
    "You want to show **parts of a whole** with a handful of categories, and a number or a label in the middle is part of the message.",
    "The reader may want to count: 62 of 100 patients, 17 of 100 servers. The cell grid makes small shares tangible in a way that an angle does not.",
    "The chart sits on a dashboard or a cover slide, where the ring shape fits the layout better than a rectangle and a little decoration is welcome.",
  ],
  avoid: [
    "You need to compare shares accurately between two charts. The cells grow outward, so equal counts do not always look equal. Use a [waffle](/charts/waffle) or a bar chart.",
    "There are more than about six categories. The slices get thin and the colours run out, at which point a sorted bar chart does a better job.",
    "A share is under one cell (1% with 100 cells). It rounds to zero cells and silently vanishes. Increase `rings` or `sectors`, or show it in a table.",
    "The values are not parts of one whole (for example, each is a separate percentage of something else).",
  ],
  read: [
    "Read the centre first: it holds the total, or **shown** if some categories are hidden.",
    "Pick a colour and count its cells. With the default 5 by 20 layout, each cell is 1% of the whole.",
    "Follow the colour around the ring. Each category is a continuous run, so you can see where one ends and the next begins.",
    "Hover a cell for the category name, its value, its share and how many cells it was given. Click a legend entry to hide a category and the rest refill the ring.",
  ],
  story: {
    title: "Ninety-seven percent of a very small ring",
    paragraphs: [
      "Priya worked in the data office of a regional hospital network, and her least favourite recurring task was the quarterly dashboard for the board. It had one slide on patient outcomes, and the slide had always been a pie chart. The board had always asked what the pie chart meant, and Priya had always explained it, and nobody had ever felt better.",
      "This quarter the slide had to answer a specific question. The network had put 400 post-surgery patients on a new follow-up protocol, and the clinical director wanted to know how many had been discharged on time, how many had been delayed, and how many had been readmitted. The honest answer was 312, 61 and 27.",
      "The pie chart had a problem there. The 27 readmissions came to 6.75 per cent of the pie, a slice the width of a pencil, and the director's eyes went straight to the 61. The readmissions were the number that mattered, and they were the slice nobody could count.",
      "Priya tried a waffle chart, which was better. But the square grid on a slide looked like an accounting spreadsheet, and the hospital's brand team had opinions. She switched to `<WarpedWaffle>` from bui-charts, passed the three outcome names and counts, and left the default five rings of twenty sectors. The 400 became 100 cells.",
      "The cells were handed out by largest remainder: 78 for on time, 15 for delayed, 7 for readmitted. The readmissions were now a clearly visible block of seven tiles at the end of the ring, and the number 400 sat in the hole where the old pie had a legend. A clinician in the second row said, without being asked, that she could count them.",
      "The board spent the meeting asking about the seven rather than the fifteen, which is where the clinical director had wanted the conversation all along. The follow-up protocol got a second ward, and the readmission rate the next quarter dropped from 6.75 to 4.5 per cent.",
      "The only complaint came from a trustee who said the chart looked like a dartboard. Priya said it was, in a sense, and that the aim was to hit the seven.",
    ],
  },
  pitfalls: [
    "**Rounding hides small values.** With 100 cells a value under 0.5% may get no cell. The total in the middle will still include it.",
    "**Cells are not equal in area.** Outer cells are bigger. Count cells, do not compare areas, and never compare two warped waffles by eye.",
    "**The ring is sorted by your input order.** Categories fill the ring in the order they appear, so sort the data if you want the biggest first.",
    "**Hiding a category re-shares the cells.** After a legend click, the remaining categories get more cells each. The middle label changes to **shown** to tell you the total no longer includes everything.",
    "**Very large holes.** A hole above about 0.7 leaves rings too thin to read. The hole is clamped at 0.9 and that is still too much.",
  ],
  related: [
    { slug: "waffle", why: "The square version: cells are equal in area and easier to compare across charts." },
    { slug: "sunburst", why: "When the parts have sub-parts and you want a ring with more than one level." },
    { slug: "mekko", why: "When the whole itself differs in size between groups and you need both size and mix." },
  ],
  media: [
    { kind: "youtube", id: "__RJP3m5zoI", title: "How to create Waffle Chart in Power BI", channel: "VisioChart", note: "Building a standard waffle chart in Power BI. The warped variant is a neighbouring chart, not covered." },
  ],
}
