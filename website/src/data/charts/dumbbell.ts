import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Two dots per item joined by a bar, so the gap between start and end is the thing you see.",
  intro: [
    "Some questions are not about how big a value is but how far it moved. Salaries before and after a review, life expectancy for men and for women, minimum and maximum temperature in each city. The answer lives in the distance between two numbers, and a pair of grouped bars is a poor way to show a distance: the eye has to compare two bar tops that are far apart, each sitting on its own pedestal.",
    "A dumbbell chart removes the pedestals. Each item is one row. A dot marks the start, another marks the end, and a thin bar joins them. The length of the bar is the change, the position along the axis is the level, and the colour of the dots tells you which end is which. It is a close relative of the [Cleveland dot plot](https://en.wikipedia.org/wiki/Dot_plot_(statistics)), which [William Cleveland](https://en.wikipedia.org/wiki/William_S._Cleveland) argued in the 1980s was a better way to show many values than bars.",
    "The result looks like barbells on a gym rack, which is either a coincidence or the whole name of the chart. Heavier dumbbells on the far right mean more of something. A short bar means nothing happened, which is also an answer.",
  ],
  math: [
    {
      heading: "Two values and a span",
      body: [
        "Each row has a name $c_i$, a start value $a_i$ and an end value $b_i$. After ordering, item $i$ takes a slot along the category axis. The sort key is the *end* value $b_i$, so `sort` ranks items by where they finished, not by where they began.",
        "Three marks are drawn per slot: a dot at $a_i$, a dot at $b_i$, and a bar covering the span between them. The bar is a bar of height equal to the absolute difference, resting on the smaller of the two values, so it works whichever way the change goes.",
      ],
      tex: [
        "\\text{span}_i = \\left[\\min(a_i, b_i),\\ \\max(a_i, b_i)\\right], \\qquad \\Delta_i = \\lvert a_i - b_i \\rvert",
      ],
    },
    {
      heading: "Details of the drawing",
      body: [
        "The span is a neutral-coloured bar at most 3 pixels thick and hidden from the legend. The dots default to 12 pixels and take the first two theme series colours, or your `start_color` and `end_color`. The legend shows your `start_label` and `end_label`, defaulting to Start and End.",
        "Horizontal is the default, so names go down the left and the axis runs along the bottom. The tooltip is a combined one for the row: start, end, and their names together, so hovering any part of the row shows both values.",
      ],
    },
  ],
  use: [
    "Each item has two values and the interesting thing is the gap: before and after, plan and actual, minimum and maximum, group A and group B.",
    "You want to rank by the later or larger of the two, and still see where each item started.",
    "You have too many items for grouped bars to stay legible, and too few time points for a line per item.",
  ],
  avoid: [
    "There are more than two values per item. A [parallel-coordinates](/charts/parallel-coordinates) plot or a small multiple suits that better.",
    "You only care whether things went up or down, not by how much. A [slope](/charts/slope) chart reads direction faster.",
    "The two values are different units. The bar between them would be meaningless.",
    "You need to show how a total is built from steps. Use a [waterfall](/charts/waterfall).",
  ],
  read: [
    "Check the legend to see which colour is the start and which is the end.",
    "Look at bar length first: long bars are the big changes, short ones are the items that did nothing.",
    "Look at where the dots sit along the axis. Two items with the same change can have very different levels, and the position tells you which.",
    "Follow the sort order. Items are ranked by the end value, so the second dot traces a clean line down the chart.",
    "Hover a row to see both numbers together.",
  ],
  story: {
    title: "The raise that went the wrong way",
    paragraphs: [
      "Tomas was the people-analytics lead at a logistics firm, which is a long way of saying he had a spreadsheet nobody else wanted to open. After the annual pay review, the head of HR asked him to show that the adjustments had moved lower-paid roles closer to the market rate.",
      "He had thirty-four job families, each with a median salary before the review and a median after. The obvious chart was clustered bars: two per family, side by side. He built it. It looked like a very orderly picket fence, and told you precisely nothing about who had moved.",
      "The problem is that each pair of bars differs by a small amount against a big shared height. Salaries are all about 40,000 to 90,000 euros, and the change was a few thousand. The bars made every raise look like a rounding error, and the picket fence hid the interesting parts under all that wood.",
      "He redrew it with `<Dumbbell>`, giving the before and after columns, sorting by the after value, and with the axis starting at 30,000. Now each family was a short bar with a dot on each end. The picture was suddenly legible.",
      "Most bars were short. A few were long. And three of the longest bars pointed the wrong way, with the after dot to the left of the before dot: warehouse supervisors, drivers' dispatchers and one family that appeared to be named \"Misc\". Those had been regraded downwards by a rule applied to a column that had been sorted differently.",
      "HR fixed the three families within a fortnight and paid back-pay of about 900 euros each to 61 people. The chart went into the report with the three wrong-way bars circled in red, which the head of HR described as the most efficient audit she had ever not requested.",
      "Bars show you how big things are. Dumbbells show you what happened to them.",
    ],
  },
  pitfalls: [
    "**Sorting by the wrong end.** `sort` ranks by the end value. If you want to rank by the size of the change, compute the change yourself and sort the rows before you pass them.",
    "**Colours without meaning.** Start and end colours are arbitrary until the legend says otherwise. Keep them the same across charts in one report.",
    "**Large axis range.** If the axis includes zero but all values are near 50,000, every bar is a sliver. Set `value_axis` to a tighter range, and say so.",
    "**Two values that are not a before and after.** For group comparisons the bar is a distance, not a change. Label the dots accordingly with `start_label` and `end_label`.",
  ],
  related: [
    { slug: "slope", why: "When direction matters more than the exact level and there are only a few items." },
    { slug: "lollipop", why: "When you have one value per item rather than two." },
    { slug: "forest", why: "When the span is a confidence interval around an estimate and not a start and an end." },
  ],
  media: [
    { kind: "youtube", id: "hmAYlCk8NXk", title: "How to Create a Dumbbell Chart in Excel", channel: "Maven Analytics", note: "Step-by-step dumbbell chart build. Uses Excel rather than this library." },
  ],
}
