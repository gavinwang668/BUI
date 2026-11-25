import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Two sets of horizontal bars growing in opposite directions from a shared centre line.",
  intro: [
    "Put two groups side by side and ask which is bigger in each category, and the standard bar chart responds with two bars per category, one slightly taller than the other, in colours that become a matter of taste. It works. It also makes you hunt for each pair in a crowd of pairs.",
    "The back-to-back bar chart turns the pair around. Both groups share one set of category rows. One group's bars grow to the left from a centre line, the other's grow to the right. You compare the two ends of a row, which is a left-versus-right judgement the eye makes without any effort, and the overall shape tells you the story before you read a label.",
    "The famous version is the [population pyramid](https://en.wikipedia.org/wiki/Population_pyramid), with age bands stacked up the middle and men on one side, women on the other. It is usually traced back to census atlases in the nineteenth century. It is also the only chart that regularly makes demographers describe a country as top-heavy.",
  ],
  math: [
    {
      heading: "Mirror one side",
      body: [
        "Each row has a name $c_i$, a left value $l_i$ and a right value $r_i$, both non-negative magnitudes. Rows keep your order, so you choose the sequence of categories. Nothing is sorted.",
        "The left series is drawn through a negation, so it grows from zero toward the negative side, and the right series grows toward the positive side. Both sit on the same value axis, which is forced to be symmetric about zero, so the centre line is in the middle.",
      ],
      tex: ["x^{\\text{left}}_i = -\\,l_i, \\qquad x^{\\text{right}}_i = r_i"],
    },
    {
      heading: "Labels that read as magnitudes",
      body: [
        "The axis really does run through negative numbers, but a tick of $-40$ on the left would be nonsense for a headcount. The axis formatter therefore prints the absolute value, so both sides read as ordinary positive magnitudes growing outward.",
        "Bars take 80% of the slot height, categories run down the left edge, and the left and right colours (the first two theme series, or your `left_color` and `right_color`) appear in the legend under your `left_label` and `right_label`. The tooltip is a combined one that shows both values for the row.",
      ],
      tex: ["\\text{tick label}(x) = \\lvert x \\rvert, \\qquad \\text{axis range symmetric about } 0"],
    },
  ],
  use: [
    "Two groups share the same categories and you want to compare them row by row: men and women by age, A and B tests by cohort, exports and imports by country.",
    "The shape of each group across the categories matters as much as individual rows, as with age profiles or response distributions.",
    "The categories are ordered (ages, scores, ratings) and the order gives the chart its silhouette.",
  ],
  avoid: [
    "The two groups are not measured in the same unit or scale. Mirroring them implies that their lengths can be compared.",
    "You need to compare one group's rows to each other precisely. Bars on the same side share a baseline, but the other group is in the way. A [lollipop](/charts/lollipop) is clearer.",
    "There are more than two groups. Use small multiples, or grouped bars.",
    "You have values that can be negative themselves. Mirroring would make the sides ambiguous. A [waterfall](/charts/waterfall) or a standard bar chart is safer.",
  ],
  read: [
    "Find the centre line: that is zero, and both sides grow away from it.",
    "Compare the two ends of one row. Which side is longer, and by how much? The axis shows the absolute value on both sides.",
    "Look at the overall silhouette. A pyramid, a column or an inverted pyramid are each a different story.",
    "Look for rows where one side nearly vanishes. That asymmetry is usually the headline.",
    "Hover a row to see both values together with the labels you chose.",
  ],
  story: {
    title: "The pyramid with a missing floor",
    paragraphs: [
      "Joan was a planner for a rural health district that covered four hundred square miles, nine villages and a surprising number of tractors. Every five years she drew the age structure of the population for the funding committee, and every five years a councillor looked at it and said it seemed fine.",
      "In the previous report the age data had been a table. Fifteen age bands, two sexes, thirty numbers in a grid that looked like a tax return. The committee approved it unread, which is the highest compliment a table can receive and the worst result for the table's author.",
      "This time Joan wanted them to see something she had noticed in the raw numbers: the twenty-to-thirty-four age band had shrunk, and one sex had shrunk far more than the other. A table can contain that fact. It cannot persuade anybody of it.",
      "She drew the data with `<BackToBack>`, age bands in the rows from youngest at the bottom to oldest at the top, men on the left and women on the right. The right shape for a healthy population is a pyramid. What she got was a barrel that narrowed sharply in the middle, with the left side noticeably shorter than the right at 25 to 34.",
      "The councillor who usually approved things unread now leaned forward. It took eleven seconds for somebody to say that the young men were leaving for work in the city, and another eleven for somebody to say that this was why the volunteer fire brigade could not fill a rota.",
      "The committee funded a pilot of subsidised housing and a small apprenticeship scheme tied to the local dairy. Two years on, the 25 to 34 male band was up by 14%, and the brigade had its rota back.",
      "A table is a place to keep numbers. A back-to-back chart is a place to disagree about them.",
    ],
  },
  pitfalls: [
    "**Both values must be positive magnitudes.** The left side is negated by the chart. If you pass already-negative numbers, they will be mirrored to the wrong side.",
    "**Counts versus shares.** If the groups have very different sizes, compare shares of each group, not raw counts, or the bigger group will always look bigger.",
    "**Order of categories.** Rows are drawn in the order given and not sorted. For ages, supply them in order, for other categories, sort them to taste.",
    "**Symmetric axis wastes space.** The axis is centred on zero, so a group that is much smaller than the other leaves empty space on its side. That is the honest picture.",
  ],
  related: [
    { slug: "dumbbell", why: "When you want to compare two values per row on a single shared axis rather than mirrored." },
    { slug: "mekko", why: "When the size of each category should also be visible, not just the split." },
    { slug: "histogram", why: "When the categories are numeric bins of a single distribution." },
  ],
  media: [
    { kind: "youtube", id: "t7YWthXWhDo", title: "How to Analyze Population Pyramids!", channel: "Mr. Sinn", note: "Shows how to read the most common back-to-back bar chart, the population pyramid." },
    { kind: "youtube", id: "IOa0V4iAprs", title: "Population Pyramids Explained", channel: "LEARN OR DIE", note: "Short explainer of reading mirrored bars side by side." },
  ],
}
