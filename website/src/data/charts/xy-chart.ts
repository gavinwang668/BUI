import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Points, lines or bubbles placed by two numbers, with size and colour for a third and fourth.",
  intro: [
    "Most questions that start with *does this go with that?* end up here. Taller people weigh more. Hotter servers fail more. Larger pull requests take longer to review, and the exceptions are the interesting part. An XY chart puts one number on the horizontal axis, another on the vertical axis, and lets your eye do what eyes are annoyingly good at: spotting the cloud, the line through it, and the one dot that is definitely not supposed to be there.",
    "The idea is Descartes' (the [coordinate plane](https://en.wikipedia.org/wiki/Cartesian_coordinate_system) carries his name, Cartesian, for good reason), and the habit of plotting measurements against each other became routine in the nineteenth century, when scientists discovered that a table of 400 numbers produces glazed eyes and a [scatter plot](https://en.wikipedia.org/wiki/Scatter_plot) produces an argument. That is the real job of the chart. It does not summarise. It shows you every row and trusts you to find the pattern.",
    "In bui-charts the XY chart is the plain, unsorted cousin of [Trend](/charts/trend). Neither axis has to be ordered, rows are drawn in the order you give them, and you can spend two more dimensions on **size** and **colour**. Four variables on one flat plot sounds like a crime. Used with restraint it is a very small, very legal one.",
  ],
  math: [
    {
      heading: "From rows to pixels",
      body: [
        "Each row $r$ gives a pair $(x_r, y_r)$. The chart finds the smallest and largest values on each axis, rounds them outwards to a *nice* range, and maps that range linearly onto the plot rectangle of width $W$ and height $H$. Hidden series are left out before the extents are computed, so the axes rescale to what you can still see.",
        "The nice range uses a step $s$ built from the raw span. With $\\ell$ and $h$ the data limits, the raw step is $(h-\\ell)/5$. Write it as $m \\cdot 10^{k}$ with $m \\in [1,10)$, then pick the smallest $m'$ from the list $1, 2, 2.5, 5, 10$ that is at least $m$. The axis runs from $\\lfloor \\ell / s \\rfloor s$ to $\\lceil h / s \\rceil s$ with $s = m' \\cdot 10^{k}$, which is why your axis ends at 250 rather than 237.",
      ],
      tex: [
        "p_x = \\frac{x - a_x}{b_x - a_x}\\,W, \\qquad p_y = \\Bigl(1 - \\frac{y - a_y}{b_y - a_y}\\Bigr)H",
        "s = m'\\cdot 10^{k}, \\quad a = \\lfloor \\ell/s \\rfloor\\, s, \\quad b = \\lceil h/s \\rceil\\, s",
      ],
    },
    {
      heading: "A third value as area",
      body: [
        "When a series has a `size` accessor, its values $v$ are normalised across **all** series together, so two series use the same scale and can be compared. With $v_{\\min}$ and $v_{\\max}$ the extremes and $[A_0, A_1]$ the `size_range` (default $5$ to $100$), a point gets",
        "The drawn diameter is the square root of that interpolated quantity, so a value twice as large covers about twice the area, not twice the width. Missing or non-finite sizes fall back to the smallest end of the range.",
      ],
      tex: [
        "t = \\frac{v - v_{\\min}}{v_{\\max} - v_{\\min}}, \\qquad d = \\sqrt{A_0 + t\\,(A_1 - A_0)}",
      ],
    },
    {
      heading: "Which point is under the cursor",
      body: [
        "Hovering is a nearest-point question, and answering it by scanning every row on every mouse move would be rude. The chart drops each point into a coarse grid of pixel cells once, then on each move checks only the cells around the cursor. A point is hit when the cursor lies within the radius $(d + w)/2$ of its centre, where $w$ is the point's stroke width, and the closest such point wins. Points lying outside the plot area are not drawn and cannot be hit.",
      ],
    },
  ],
  use: [
    "Both axes are **measurements**: load against latency, dose against response, price against area.",
    "You want to see clusters, outliers or correlation before you decide what to say about them.",
    "A third value (a size) or a fourth (a colour) explains the cloud: the big dots are the big customers, the red ones are failing.",
    "The order of rows is the story, as in a path, a trajectory or a hysteresis loop. Lines follow your row order exactly.",
    "You need one chart for many groups: one `<Series>` per group, each with its own colour, sharing the same axes.",
  ],
  avoid: [
    "x is a category such as team or region. A bar chart gives each category a slot and a label. A scatter plot gives you ugly numbers on an axis that means nothing.",
    "x is time and the question is how a value moved. [Time Series](/charts/time-series) has a time axis, sensible ticks and zoom that understands clocks.",
    "You have tens of thousands of points that sit on top of each other. Lower `fill_opacity` helps up to a point. Past it, bin the data and use a [Heatmap](/charts/heatmap).",
    "Size and colour carry different variables on every series. Readers will compare them as if they were the same thing, and then it is your fault.",
  ],
  read: [
    "Look at the overall shape first: a rising cloud, a flat one, two blobs, a curve. The shape is the finding. Individual points are footnotes.",
    "Find the outliers and ask whether they are errors or the story. A dot far from the cloud is either a bug in the data or a customer worth calling.",
    "If points have different sizes, remember that *area* tracks the value. A dot twice as big is twice the value, not four times.",
    "Hover a point for its x and y, plus its size and colour value when the series has them. Click a legend entry to isolate a series, and Ctrl-click (Cmd-click on macOS) to toggle just one.",
  ],
  story: {
    title: "Forty-one goats and one very honest scatter plot",
    paragraphs: [
      "Mirela ran quality at a small cheese cooperative in the hills above Sibiu. The cooperative bought milk from forty-one farms, and for years the only thing anyone knew about those farms was the price per litre and whether the farmer would pick up the phone.",
      "Then a batch of aged cheese came back from a distributor with a note that said, in the polite way these notes say things, that it tasted like a wet barn. Mirela had a spreadsheet with a row per farm: average somatic cell count in the milk, fat percentage, litres delivered per week. She also had a boss who wanted to know by Friday whether this was one bad farm or a pattern.",
      "She started with the thing everyone starts with, a pivot table, and stared at it for an afternoon. Forty-one rows of numbers is an excellent way to feel busy.",
      "On Thursday she put the data into an `<XYChart>` from bui-charts. Cell count on the x axis, fat percentage on y, weekly litres as the point size, and a colour ramp from green to red for the number of failed taste panels. It took about twenty lines of Rust, most of which were opening the CSV.",
      "The cloud had a clear tilt, which she half expected: higher cell count, lower fat. What she did not expect was one enormous red dot sitting alone at the top right, a farm that delivered more milk than any other and failed nearly every panel. Farm number 17 was also the only one that had been added in the last year, and the only one with a new milking shed that nobody had inspected.",
      "The inspection found a cleaning schedule that existed mainly as a laminated sheet on the wall. After the fix the farm's dot slid left and down over two months, and the failed panels dropped from nine in a quarter to one. The distributor stopped writing notes about barns.",
      "Mirela keeps the first version of the chart pinned above her desk. Under it she wrote one sentence: forty-one rows of data, one dot that mattered, and a pivot table that had been hiding it for years.",
    ],
  },
  pitfalls: [
    "**Lines are drawn in row order.** If your rows are not sorted by x and you ask for lines, you get a scribble. Sort first, or use [Trend](/charts/trend), which does it for you.",
    "**Overplotting.** A thousand dots stacked in the same place look like one dot. Lower `fill_opacity` so dense areas read darker.",
    "**Size range too wide.** With the default 5 to 100 range, one huge value can bury its neighbours. Narrow `size_range` when the biggest bubble covers half the plot.",
    "**Colour without a scheme choice.** A rainbow for a one-sided value suggests categories that are not there. Use a single-hue gradient for how much, thresholds for ok, warning, critical.",
    "**Data as a plain `Vec`.** Passed directly, the per-row work is redone on every render of your component. Keep large data in a signal.",
  ],
  related: [
    { slug: "trend", why: "When x always goes up and you want lines joined in x order, with stacking and a shared tooltip." },
    { slug: "heatmap", why: "When there are so many points that counting them per cell tells you more than drawing them." },
    { slug: "beeswarm", why: "When one axis is a category and you want every point visible without overlap." },
    { slug: "time-series", why: "When x is time." },
  ],
  media: [
    { kind: "youtube", id: "3d3Lx-Jz_kM", title: "New: XY Chart is Generally Available in Grafana 11.1 | Grafana", channel: "Grafana", note: "Grafana announcement and demo of the XY chart panel, which this chart is modelled on." },
  ],
}
