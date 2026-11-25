import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Three parts that add up to a whole, plotted as a point inside a triangle.",
  intro: [
    "Some things come in threes and add up to everything: sand, silt and clay in a soil. Three metals in an alloy. The share of votes going to three parties that between them took the lot. If you know two of the shares, the third is whatever is left, so the data has only two degrees of freedom and a flat page can hold it. The trick is to pick the right flat shape, and for three parts it is a triangle.",
    "In a ternary diagram each corner stands for 100% of one component. A point at a corner is pure. A point on an edge has none of the opposite corner. A point in the exact middle is one third of each. Soil scientists have used this since long before computers, and the soil texture triangle that sorts a sample into \"loam\", \"clay\" or \"sandy loam\" is probably the most-used ternary diagram on Earth, by a margin that a petrologist would find embarrassing.",
    "It looks like magic until you try to read one for the first time, at which point it feels like a cryptic crossword. The grid lines run in three directions, and which direction belongs to which corner is the whole puzzle. Once you have it, though, the chart does something few charts can: it lets you compare mixtures, and see which are close, as a distance on the page.",
  ],
  math: [
    {
      heading: "Normalising the three parts",
      body: [
        "A row gives three non-negative amounts $a$, $b$, $c$, in any unit. Only their proportions matter, so the chart divides by their sum. Rows where the sum is not finite are skipped. A row whose total is zero has no composition, and is drawn at corner A.",
      ],
      tex: [
        "\\hat a = \\frac{a}{a + b + c}, \\quad \\hat b = \\frac{b}{a + b + c}, \\quad \\hat c = \\frac{c}{a + b + c}, \\qquad \\hat a + \\hat b + \\hat c = 1",
      ],
    },
    {
      heading: "Barycentric coordinates to pixels",
      body: [
        "The triangle is equilateral with side $s$, fitted into the available area as large as it will go. Component A sits at the bottom left (the origin $(x_0, y_0)$), B at the bottom right and C at the top. A point is the [weighted average of the three corners](https://en.wikipedia.org/wiki/Barycentric_coordinate_system), with the shares as weights, which is the same as the formula below (with $y$ pointing down in screen space).",
      ],
      tex: [
        "x = x_0 + s\\left(\\hat b + \\tfrac{1}{2}\\hat c\\right), \\qquad y = y_0 - s\\,\\tfrac{\\sqrt{3}}{2}\\,\\hat c",
      ],
    },
    {
      heading: "The grid and how to read a share",
      body: [
        "With `divisions` set to $d$ (10 by default) the chart draws $d - 1$ grid lines in each of the three directions at shares $k/d$, labelled in percent. The line of constant $\\hat c$ is horizontal. The line of constant $\\hat b$ runs parallel to the side opposite B, and the line of constant $\\hat a$ runs parallel to the side opposite A. A share is read by following the line parallel to the side opposite the corner. Hidden groups are removed from the picture; their points do not affect the normalisation of the others, since each row is normalised on its own.",
      ],
    },
  ],
  use: [
    "Your data is **three parts of a whole**: composition, share, texture, blend.",
    "You want to compare many samples at once and see clusters, trends and outliers in composition.",
    "Mixing matters. A mixture of two samples falls on the straight line between them, which no other chart shows this cleanly.",
    "You are working in a field that already uses the triangle, such as soil science, petrology, alloys or polymer blends. Your readers will already know how to read it.",
  ],
  avoid: [
    "You have **four or more** components. A triangle cannot hold them. Group some, or use a [parallel coordinates](/charts/parallel-coordinates) chart.",
    "The total is meaningful. A ternary hides it, because it normalises every row. Show the totals elsewhere.",
    "Your audience will read exact values. Triangular grids are hard to read by eye, so offer a table too.",
    "Two of the three components are nearly always zero. Every point crowds into one corner, and a bar chart would work better.",
  ],
  read: [
    "Find the corner labels. A point close to a corner is mostly that component.",
    "To read a share for one component, follow the grid line through the point that runs *parallel to the opposite side* and read its label on the edge.",
    "Look at the cloud: a tight cluster is a consistent mixture, a streak along a line is a two-component mixture, and a point far from the rest is an outlier worth checking.",
    "Hover a point for its name and its three shares to one decimal. Click a legend entry to hide a group.",
  ],
  story: {
    title: "The hill that got all the credit",
    paragraphs: [
      "Ines advised a vineyard in a region that was proud of its soil in the way some families are proud of a distant relative. The estate had sixty-two plots, a founder's legend that the best grapes came from the \"clay hill\" and a new owner who had read that the best soil for wine was a lot of things and was determined to find out which.",
      "She had lab results for every plot: sand, silt and clay, in percent. The owner kept asking which plots were similar to the clay hill, and she kept answering with a table of three columns of numbers, which he looked at the way people look at a train timetable in a foreign language.",
      "The numbers were fine. The question was one of distance, and three columns are a terrible way to measure distance. Two plots with 40/35/25 and 38/37/25 are basically the same, but nobody sees that in a table. A plot with 12/20/68 is not, but it takes ten seconds of arithmetic to confirm.",
      "She fed the results into a `<Ternary>` from bui-charts, with corners named Sand, Silt and Clay, and the plot block as the `group`, so the vineyard's four sections each had a colour. The founder's clay hill sat near the top corner, as promised, with three other plots next to it. Then the surprise: the best-selling wine of the last decade came from a cluster that was nowhere near it, a wide patch of silt-rich plots on the lower slope.",
      "The legend was right and the legend was wrong. Clay hill was clay. The best wine was silt. Over a tasting that evening, the owner took the news with remarkable composure, which in his case meant opening a second bottle.",
      "They ran soil tests on the eight unplanted plots, and found that two of them, left empty for decades because they looked unremarkable, fell right in the silt cluster. They planted them the next spring, and three years later the first vintage from them sold out at a price 18% above the estate average.",
      "The triangle did not know anything about wine. It simply refused to let a legend outvote the data, which is what a good chart does at its best.",
    ],
  },
  pitfalls: [
    "**Reading the wrong grid direction.** Each of the three line families goes with a different corner. Label the corners clearly and give them units in the caption.",
    "**Closure effects.** Because shares add up to 1, if one goes up another must go down. Correlations you see between them are partly forced by the geometry.",
    "**Rows that are not whole mixtures.** If your three columns are only part of the sample (the rest is organic matter, say), the chart shows the ratio of the three, not the sample.",
    "**Tiny clusters.** If every point lies in the middle ten percent of the triangle, the picture has no resolution. Subtract the minimum from each share and rescale, or just zoom the data in a different tool.",
  ],
  related: [
    { slug: "ternary-contour", why: "When you have a formula or model over the whole triangle instead of a set of samples." },
    { slug: "piper", why: "Two ternary triangles and a diamond joined up, built for the six main ions in water." },
    { slug: "waffle", why: "When you only need the shares of one whole, with no comparison between samples." },
  ],
  media: [
    { kind: "youtube", id: "DHeJ_KBqEuc", title: "Ternary (triangle) plot. What is it and how to read it?", channel: "Scientific Illustrations", note: "A short walk-through of the three grid directions and how to read a composition from a point." },
  ],
}
