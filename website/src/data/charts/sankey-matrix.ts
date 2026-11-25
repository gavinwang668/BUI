import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A grid of squares sized by value, with a shrinking band behind each row showing how its total is spent as you move across.",
  intro: [
    "A [Sankey diagram](https://en.wikipedia.org/wiki/Sankey_diagram) is lovely until somebody asks what the third thin ribbon from the top is worth. Ribbons are good at showing which flows are big and bad at telling you the exact number, and they are hopeless at letting you compare a ribbon in one place with a ribbon far away from it. People nod at Sankeys and then ask for the spreadsheet.",
    "A sankey matrix is the spreadsheet, drawn so that it still tells the Sankey story. Rows are sources, columns are targets, and each cell that has a flow holds a square whose area is that flow. Behind each row, a pale band starts at the full width of the row's total and narrows as it passes each column, losing exactly the amount that was drawn there. You get the flow and the grid in one picture.",
    "The result sits between a heatmap and a Sankey. It is less pretty than the ribbons and a good deal more honest, because every number sits in its own cell, lined up with its neighbours, ready to be compared.",
  ],
  math: [
    {
      heading: "Rows, columns and the matrix",
      body: [
        "Each row of data has a source, a target and a value. Sources become matrix rows and targets become matrix columns, in order of first appearance. Rows with a value that is zero, negative or not finite are dropped. Rows with the same source and target are added into one cell $v_{ij}$.",
        "From the cells the chart computes the row totals $R_i$, the column totals $C_j$ and the grand total $T$. Hidden rows (turned off in the legend) are removed before any totals are computed, so everything else re-scales.",
      ],
      tex: ["R_i = \\sum_j v_{ij}, \\qquad C_j = \\sum_i v_{ij}, \\qquad T = \\sum_i R_i = \\sum_j C_j"],
    },
    {
      heading: "Squares: area is the value",
      body: [
        "The drawing area is divided into equal cells, $w$ by $h$ pixels each. The largest square fits the smaller cell dimension less a 4 px margin, $c = \\min(w, h) - 4$, and every other square is scaled from it by the square root of its value relative to the largest cell. The square root is what makes the **area**, not the side, proportional to the value, which is the right thing to compare.",
      ],
      tex: ["s_{ij} = c\\,\\sqrt{\\frac{v_{ij}}{\\max_{kl} v_{kl}}}"],
    },
    {
      heading: "The shrinking band and the column bars",
      body: [
        "Behind row $i$ is a band whose thickness at the left edge is proportional to the row total. After column $j$ it has lost the value drawn in that column, so its thickness at the right edge of column $j$ is the amount still unaccounted for. The thickness unit is the same for every row (the tallest allowed row height over the busiest row total), so bands from different rows can be compared.",
        "Above the grid, a small bar per column shows the column total, scaled so the largest takes 34 px. Together the band and the bars are the Sankey in miniature: what leaves each row on the left, what arrives in each column along the top.",
      ],
      tex: ["\\text{remaining}_{i}(j) = R_i - \\sum_{k \\le j} v_{ik}"],
    },
  ],
  use: [
    "You want a **flow diagram you can read numbers from**: budget by department and category, traffic by source and landing page, orders by warehouse and carrier.",
    "There are many pairs and many cells, so ribbons would cross and tangle. A grid never does.",
    "The reader will compare cells in different places: is the biggest cost for engineering larger than the biggest for sales? Squares line up for that in a way ribbons never can.",
    "You want a table with a bit of visual emphasis for a mixed audience. Each cell shows its value if the square is big enough.",
  ],
  avoid: [
    "The flow passes through more than two stages. A matrix has exactly two sides. Use [alluvial](/charts/alluvial) or a [sankey](/charts/sankey).",
    "You want the emotional punch of a big ribbon. A square is calm. That is a strength in a review and a weakness in a keynote.",
    "There are only a handful of pairs. A small [sankey](/charts/sankey) will be cleaner and easier to explain.",
    "All you need is a colour-coded table. A [heatmap](/charts/heatmap) is more direct.",
  ],
  read: [
    "Find a row and read across: each square is where that row's total went, and its area is the amount.",
    "Watch the pale band behind the row. It starts at the row's full total and narrows after each square, down to nothing at the last column.",
    "Read down a column to see which rows feed it. The small bar above the column is its total.",
    "Hover a square for its value and its share of its row, its column and the whole. Click a legend entry to hide a row. Everything re-scales.",
  ],
  story: {
    title: "Seventeen ribbons and one question",
    paragraphs: [
      "Anselm worked in procurement at a regional airline called Skylark, and every quarter he built a Sankey diagram of spend: from departments on the left to suppliers on the right. He had built them for years. They looked splendid on the wall of the finance floor, and nobody had ever read a number off one.",
      "On a recent Thursday the chief financial officer came by, looked at the wall and asked what the airline spent with its largest fuel supplier on behalf of the engineering department. Anselm squinted. The ribbon was there. It was the fourth from the top and it crossed three others. It was somewhere between a lot and a little.",
      "The question was reasonable, the answer was in a spreadsheet three clicks away, and the chart had made it harder to find. He realised the Sankey was a fine way to show that spend was large and a poor way to say how large. What he needed was something where every number had its own address.",
      "He changed the chart to `<SankeyMatrix>` from bui-charts. Same rows of department, supplier and amount, no other changes. The ribbons disappeared and a grid of squares replaced them, with departments down the side, suppliers across the top and amounts printed inside the big squares.",
      "The answer to the CFO's question was a square near the top left, labelled 4.1 million. Her second question, whether that was more than the maintenance spend with the same supplier, took one glance down the column: no, it was about half. The whole conversation took less time than the walk to the wall.",
      "The matrix also showed something nobody had asked about. The pale band behind the catering row narrowed to a point after just two columns, while the one behind engineering spread across nine. Catering bought from two suppliers. Engineering bought from nine, three of which sold the same part. That led to a supplier consolidation that, a year later, had saved 640,000.",
      "Anselm still makes a Sankey for the wall. He now keeps the matrix next to it, for when somebody asks a question.",
    ],
  },
  pitfalls: [
    "**Area, not side.** A square with twice the side is four times the value. This is correct and is also how the eye misjudges squares, so print the values or rely on the tooltip.",
    "**Scale is global.** The largest cell sets the scale. One huge flow makes every other square small, and small squares may be too small to carry a label.",
    "**Order is first appearance.** Rows and columns appear in the order they are first met in the data, not by size. Sort the input if you want the largest on top.",
    "**Duplicated pairs add up.** Two rows with the same source and target merge into one cell, which is usually what you want and sometimes a surprise.",
    "**The band narrows to the right.** It is a running remainder, not a trend. A band that narrows quickly just means the row's flow is concentrated in the first columns.",
  ],
  related: [
    { slug: "sankey", why: "When you want the shape of the flow and a few big ribbons matter more than exact values." },
    { slug: "heatmap", why: "When colour is enough and you do not need the flow band." },
    { slug: "chord-diagram", why: "When flows go both ways between the same groups." },
  ],
  media: [
    { kind: "youtube", id: "AQFiGzVjOsY", title: "Sankey diagrams mapping flows between connected processes intuitively", channel: "Data Science With Dennis", note: "General introduction to Sankey diagrams mapping flows between connected nodes, not specific to the matrix layout." },
  ],
}
