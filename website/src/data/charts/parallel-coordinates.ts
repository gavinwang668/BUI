import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "One vertical axis per measure and one polyline per row, so patterns across many measures show up as bundles of lines.",
  intro: [
    "A scatter plot is a marvellous invention that stops working at the third dimension. Add a colour for a fourth, a size for a fifth, and by the sixth you are explaining the legend to someone who has stopped listening. Real data usually has ten columns, not two, and you would like to look at all of them at once without building a small museum.",
    "[Parallel coordinates](https://en.wikipedia.org/wiki/Parallel_coordinates) do it with a change of viewpoint. Instead of putting the axes at right angles, lay them out side by side, all vertical, all parallel. A row of data is then a path: it crosses the first axis at its value for the first measure, the second axis at its value for the second, and so on, and you connect the dots with a line. A thousand rows are a thousand lines, and any two measures can be compared by looking at what the lines do between neighbouring axes.",
    "The idea was developed into a working tool by [Alfred Inselberg](https://en.wikipedia.org/wiki/Alfred_Inselberg), who spent decades showing that a point in many dimensions becomes a line in this picture and a line becomes a point. For you, the practical version is simpler. If the lines between two axes run parallel, the measures rise and fall together. If they criss-cross in an X, one goes up as the other goes down. If they bundle into a thick ribbon, you have found a cluster.",
  ],
  math: [
    {
      heading: "One scale per axis",
      body: [
        "Take $m$ rows and $p$ axes. Each axis has its **own** range, set from the data: the smallest and largest finite values in that column, written $\\ell_a$ and $u_a$. That is the main difference from a [spider chart](/charts/spider), where all spokes share one scale, and it is why a column in grams and a column in euros can sit next to each other.",
        "A value $v_{ia}$ becomes a share of the axis height, and the polyline for row $i$ goes through the points below. If an axis has no spread (all values equal, $u_a = \\ell_a$), every row is drawn at the middle of that axis. The top and bottom of each axis are labelled with $u_a$ and $\\ell_a$.",
      ],
      tex: [
        "s_{ia} = \\frac{v_{ia} - \\ell_a}{u_a - \\ell_a}, \\qquad \\bigl(x_a,\\; y_{ia}\\bigr) = \\Bigl(X_0 + \\tfrac{a}{p-1}\\,W,\\;\\; Y_{\\text{bottom}} - s_{ia}\\,H\\Bigr)",
      ],
    },
    {
      heading: "Lines, gaps and groups",
      body: [
        "Axes are spaced evenly from left to right, in the order you list them. That order is not decorative: the chart only shows relationships between *neighbouring* axes clearly, so the same data with the axes reshuffled reveals different patterns. Related measures should sit next to each other.",
        "A value that is missing or not finite breaks the line: the row is drawn as separate runs on either side of the gap instead of being joined through it. Lines are drawn at $0.6$ opacity and $1.5$ pixels by default so that dense areas become darker where many rows overlap, which is how you see a bundle.",
        "Colour comes from the `group` function if you give one, so all rows of the same class share a colour and the legend lists the groups. Without it, every row gets its own colour and the legend is hidden. Hovering a line draws it on top at full strength and dims the rest, which is how you read one row among many.",
      ],
    },
    {
      heading: "Why the lines cross",
      body: [
        "Between two neighbouring axes, two rows cross exactly when their order on one axis is the reverse of their order on the next. Lots of crossings mean the two measures are negatively related, and almost no crossings mean they are positively related. That is a rank statement, not a linear one, and it is the intuition behind the picture.",
      ],
    },
  ],
  use: [
    "You have **many rows across several measures**, say five to fifteen, and want to look for clusters, trends and outliers at once.",
    "The measures are in different units and you have no wish to rescale them: each axis has its own range.",
    "You have a class label and want to see which measures separate the classes: colour by `group` and look for axes where the colours do not overlap.",
    "You want a quick overview before choosing which pairs to look at in a scatter plot.",
  ],
  avoid: [
    "You only have two or three measures. A scatter plot is simpler and more accurate.",
    "You have tens of thousands of rows. The lines overprint into a solid block. Bin them first, or sample.",
    "The axes cannot be put in a meaningful order. The chart is sensitive to adjacency, and a bad order hides what is there.",
    "You need to compare absolute heights across axes. Each axis has its own scale, so only the shapes and the relative positions are comparable.",
  ],
  read: [
    "Read the axis labels first: the top number is the maximum for that measure and the bottom is the minimum. Each axis has a different scale.",
    "Look at the gaps between neighbouring axes. Parallel lines mean the measures move together. Crossing lines mean they move in opposition.",
    "Look for thick bundles of one colour. A bundle is a cluster, and when it separates from other colours on some axis, that measure tells the classes apart.",
    "Find the lone line that does something different from the rest: that is an outlier, and it is easy to spot here.",
    "Hover a line to read the row and bring it to the front. Click a legend entry to isolate a group.",
  ],
  story: {
    title: "Eleven wines and a stubborn hunch",
    paragraphs: [
      "Rosalind was a junior chemist at a small winery whose owner, Mr. Okafor, believed with great sincerity that the wines from the north slope were better. He could not say why. He said \"structure\" a lot and gestured with a glass, and the lab had an assignment to find out whether chemistry agreed.",
      "The lab measured eight things in every batch: alcohol, acidity, residual sugar, tannin, pH, sulphur dioxide, colour intensity and density. There were 190 batches over six harvests, each tagged by slope. Eight columns of numbers in different units, and an owner who disliked scatter-plot matrices on the grounds that there were too many of them.",
      "The task was to see, across all eight measures at once, whether the batches from the two slopes were different in any way that a person could see. That is a question about many dimensions and two groups, and the type of picture that makes a hunch testable.",
      "She put the 190 rows into `<ParallelCoordinates>` from bui-charts, with the eight measures as `axes`, the slope as `group`, and the opacity turned down to 0.35 so that overlaps would show. The axes were already in a sensible order, and she moved sugar next to acidity because they tend to trade off.",
      "North slope lines came out as a tidy teal ribbon that ducked low through acidity, sat high on tannin and bunched hard on colour intensity. The south slope was a loose orange cloud with no commitments. At tannin and colour the two groups barely overlapped. Mr. Okafor had not been wrong, and he looked disturbed by how much he enjoyed it.",
      "There was also one lone line, a north slope batch with a sulphur dioxide reading three times higher than any other. It had been mislabelled. The sample was from a barrel that had been treated for a mould problem. Rosalind removed it, and the lab added a check for the tag.",
      "The north slope got a label that said \"high tannin, deep colour\" and the owner stopped saying \"structure\" for nearly a week.",
    ],
  },
  pitfalls: [
    "**Axis order is part of the answer.** Patterns show up between neighbours. If two related measures are at opposite ends, their relationship will not be visible. Reorder before concluding there is nothing there.",
    "**Every axis has its own range.** The heights on different axes are not comparable. A line that is high on two axes is high *relative to each axis's range*, not equal in size.",
    "**Outliers stretch an axis.** One extreme value expands that axis and flattens everything else. Remove or clip it if it is an error.",
    "**Too many lines is a solid block.** Lower `line_opacity`, narrow `line_width`, or group rows by class before drawing.",
    "**Missing values split the line.** A gap in a row's data breaks its line, rather than joining across it.",
  ],
  related: [
    { slug: "spider", why: "When there are only a few items to compare and every measure shares the same scale." },
    { slug: "dendrogram", why: "When you want to find the groups first and then look at what separates them." },
    { slug: "heatmap", why: "When you want a compact grid of values and not lines." },
  ],
  media: [
    {kind:"youtube",id:"Sb5dPdBVuNQ",title:"Parallel Coordinates: The Best Plot You’re Not Using (Yet)!",channel:"AnalytiCode",note:"Motivates parallel coordinates for multivariate data and shows how to read them."},
    {kind:"youtube",id:"JUfTNNlv97A",title:"Introduction to Parallel Coordinates-A Tutorial (Using Slides and Software)",channel:"Bob Laramee",note:"Academic-style tutorial on the technique, its strengths and pitfalls."},
  ],
}
