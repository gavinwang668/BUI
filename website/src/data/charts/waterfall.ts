import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Shows how a starting value becomes an ending value through a series of increases and decreases.",
  intro: [
    "Profit last year was 1.2 million. Profit this year is 1.5 million. Somebody asks why, and you open your mouth to say \"several things\", which is the answer that ends careers in finance. The honest answer is a list: revenue up this much, costs down that much, one regrettable write-off. A waterfall chart is that list with the arithmetic drawn in.",
    "Each bar is a change, and it floats. It starts where the previous bar ended, climbs or falls by its own amount, and the next bar picks up from there. The first bar starts at zero. At the end, a total bar sits on the baseline showing where you finished. Consultancies took to it with great enthusiasm, and you can see why: it is the one chart that makes subtraction look like a narrative.",
    "It is also called a [bridge chart](https://en.wikipedia.org/wiki/Waterfall_chart), particularly when the thing being bridged is a year-on-year number, which is when everyone wants a bridge and nobody has one.",
  ],
  math: [
    {
      heading: "A running sum",
      body: [
        "The data is a list of changes $\\delta_1, \\dots, \\delta_n$ in your order, not running totals. The chart keeps a running value $S_i$, starting at $S_0 = 0$. A change that is not a finite number counts as zero.",
        "Bar $i$ spans from $S_{i-1}$ to $S_i$. It is an increase when $\\delta_i \\ge 0$ and a decrease when $\\delta_i < 0$, and in both cases its height is $\\lvert \\delta_i \\rvert$ and its bottom is the smaller of the two ends.",
      ],
      tex: [
        "S_i = S_{i-1} + \\delta_i, \\qquad S_0 = 0",
        "\\text{bar}_i = \\left[\\min(S_{i-1}, S_i),\\ \\max(S_{i-1}, S_i)\\right]",
      ],
    },
    {
      heading: "The total bar",
      body: [
        "With `total` on (the default), one more bar is added at the end with the label from `total_label` (Total by default). It has height $\\lvert S_n \\rvert$ and sits on the baseline, extending upward when $S_n \\ge 0$ and downward when it is negative.",
        "Increases use the theme's good colour, decreases its bad colour, and the total uses the first series colour. Each can be overridden with `increase`, `decrease` and `total_color`. Value labels are shown on bars by default. Because every bar is positioned by its floor, a running sum that crosses zero still draws correctly.",
      ],
      tex: ["\\text{total} = \\left[\\min(S_n, 0),\\ \\max(S_n, 0)\\right]"],
    },
    {
      heading: "Orientation",
      body: [
        "The default is vertical, with steps left to right, because a waterfall reads like a sentence. Set `horizontal={true}` when step names are long. The chart does no sorting: the order of the steps is the order of your rows, and a different order gives a different picture of the same sum.",
      ],
    },
  ],
  use: [
    "You need to explain a difference between two numbers: this year's profit against last year's, a budget against an actual, a baseline against a forecast.",
    "The contributions are signed, a mix of gains and losses that net out to a total.",
    "The audience is going to ask \"why?\" the moment they see the final number, and you would rather answer first.",
  ],
  avoid: [
    "You have too many steps. Past about 12 bars, the narrative dissolves. Group the small ones into an \"Other\" step.",
    "The steps are not additive: percentages, ratios or averages cannot be summed this way.",
    "You want to compare steps to each other, not to the running total. A [lollipop](/charts/lollipop) or bar chart gives a common baseline for that.",
    "You only have a start and an end for each item. That is a [dumbbell](/charts/dumbbell) or a [slope](/charts/slope), not a waterfall.",
  ],
  read: [
    "Start at the left. The first bar rises from zero by the first change.",
    "Follow the floating bars. Green climbs and red falls, and each one begins where the last one ended.",
    "Read the final bar. That is the sum of every step above it.",
    "If a bar crosses the baseline, the running value went through zero there.",
    "Hover a bar for its name and value.",
  ],
  story: {
    title: "The 300k that left in March",
    paragraphs: [
      "Rafael was the head of finance at a regional furniture maker, and on the second Thursday of January he was going to be asked about the operating profit. It had risen from 2.1 to 2.4 million, a result that made the owners cheerful and made Rafael nervous, because he knew about March.",
      "March was when the factory had replaced its largest customer's order book with a smaller one, and the loss had been absorbed by an unusual run of cheap timber in the summer. The two had cancelled out so neatly that the profit line was fine, and the underlying business was less fine.",
      "A slide showing 2.1 and 2.4 would not carry that. Neither would a table of twelve line items, each of which was individually boring. Rafael needed a picture in which the good news and the bad news were both visible and both added up.",
      "He took the year's movements, 11 of them in the end, and passed them to `<Waterfall>` as changes with the default total bar. The timber saving appeared as a big green step of 0.9 million. The lost customer was a big red one of 0.7. Between them were nine smaller bars that nobody would have remembered by name.",
      "The final column was 0.3 million higher than the start. The two giant bars beside each other in different colours told the story before he spoke. One of the owners, who normally asked about the total, asked instead about the red bar.",
      "That conversation led to a new contract with two mid-sized hotel chains, signed in April, which had been talked about for two years and never prioritised. The following year's waterfall had a green step of 0.6 million labelled with the name of one of the chains.",
      "A total says how things ended. A waterfall says what happened on the way, and who left the room.",
    ],
  },
  pitfalls: [
    "**Pass changes, not totals.** If you give it running totals, the chart will add them up again and the bars will be nonsense. The final total bar is computed for you.",
    "**Order changes the picture.** The sum is the same however you order the steps, but the shape is not. Put the big movements first or last, and explain the order you chose.",
    "**A zero change is drawn as an increase.** It has no height, so it appears as a label with no bar. Drop zero rows if they distract.",
    "**Mixing units.** Every step must be in the same unit as the total. A step in millions next to a step in thousands will vanish.",
    "**Too many small bars.** If most steps are tiny, the interesting ones are lost. Collapse the tail into one step.",
  ],
  related: [
    { slug: "funnel", why: "When the question is how many remain after each stage rather than how a total is built up." },
    { slug: "dumbbell", why: "When you only need the gap between a start and an end for each of many items." },
    { slug: "sankey", why: "When the changes are flows from one set of things into another." },
  ],
  media: [
    { kind: "youtube", id: "8hXSAbalZuw", title: "Why Consultants like McKinsey Use Waterfall Charts", channel: "Leila Gharani", note: "Explains why consultants use waterfall charts for bridging values." },
    { kind: "youtube", id: "kLKwGNpdOHM", title: "⬇️ How to Read Waterfall Charts", channel: "Chartosaur", note: "A concise guide to reading waterfall charts." },
  ],
}
