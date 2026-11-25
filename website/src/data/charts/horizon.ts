import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A line chart sliced into bands and folded down, so a dozen series fit where one used to.",
  intro: [
    "Dashboards have a standing problem: forty series, one screen, and a line chart that is about as readable at that density as a plate of noodles. Making each line chart shorter does not help for long, because once a line is 30 pixels tall the wiggles are lost in the pixel grid.",
    "The [horizon chart](https://en.wikipedia.org/wiki/Horizon_chart) is a way to cheat the height. Cut the vertical range of a series into equal slices, tint each slice a bit darker than the one below it, and then lay all of the slices on top of one another in a single short row. The tallest peaks are now the darkest patches, and the whole row is a quarter, or a third, of its original height. The technique was introduced for time series by Saito and colleagues in the mid-2000s, and tested rather carefully by Jeffrey Heer and his co-authors in a 2009 paper called [*Sizing the Horizon*](https://doi.org/10.1145/1518701.1518897), which is a title no other chart has earned.",
    "The price is a small amount of learning. Nobody reads a horizon chart correctly on the first glance. But after a minute of explanation (darker means larger) the eye picks it up, and the chart becomes the best tool there is for scanning a lot of series for the one that did something.",
  ],
  math: [
    {
      heading: "Slicing a series",
      body: [
        "Each series is a sequence $y_t$. For a series, find its largest absolute value $m = \\max_t |y_t|$ and divide it by the number of bands $B$ (default 3) to get the band height $\\Delta = m / B$. Every series gets its own $\\Delta$, so a series that moves between $-2$ and $2$ and one that moves between $-2000$ and $2000$ both use the full depth of their rows. The band height is printed under the series name.",
        "If the series is all zeros or empty, $\\Delta$ falls back to 1.",
      ],
      tex: ["\\Delta = \\frac{\\max_t |y_t|}{B}"],
    },
    {
      heading: "Folding",
      body: [
        "Band $k$ (counting from $0$) holds the part of the value that falls inside the $k$-th slice. Positive values fill the positive bands and negative values, mirrored, fill their own, so a single row shows both signs. Each band is a layer of height at most $\\Delta$, and every layer is drawn on the same axis from $0$ to $\\Delta$, one on top of the next.",
        "A value that is missing is left blank in every band, not drawn as zero.",
      ],
      tex: [
        "p_k(t) = \\min\\!\\bigl(\\Delta,\\; \\max(0,\\; y_t - k\\Delta)\\bigr), \\qquad n_k(t) = \\min\\!\\bigl(\\Delta,\\; \\max(0,\\; -y_t - k\\Delta)\\bigr)",
      ],
    },
    {
      heading: "Shade",
      body: [
        "Layer $k$ is filled with an opacity that grows with $k$, so the deepest slice is the darkest. Positive layers use the `positive` colour (the theme's good colour by default) and negative layers use `negative` (the bad colour). With the default $B = 3$ the opacities are about 0.53, 0.77 and 1.0.",
        "More bands make each row shorter for the same range but harder to read, because the eye has to tell more shades apart. Three is a sensible start. Two is nearly a plain area chart.",
      ],
      tex: ["\\alpha_k = 0.3 + 0.7\\,\\frac{k + 1}{B}, \\qquad k = 0, \\dots, B - 1"],
    },
  ],
  use: [
    "You have many series over the same time axis and not enough vertical room: servers, sensors, stocks, sales regions.",
    "You are scanning for the one series that behaves differently, and a picture of ten rows in the height of two lines makes it easier to find.",
    "Your data has both signs and a meaningful zero: deltas, returns, deviations from a target.",
    "The viewer will see the chart often enough to learn it. A dashboard on a wall is the ideal setting.",
  ],
  avoid: [
    "You need to read exact values. Layers tell you the order of magnitude, not the number. Use a [time series](/charts/time-series) for that.",
    "You have three series and plenty of room. A plain line chart is clearer, and the horizon's compactness buys you nothing.",
    "The audience will see it once and has no time to learn the encoding. A chart that needs a two-line legend to read is a bad one for a single slide.",
    "You need to compare magnitudes across series. Each series has its own band height, so a dark row in one series is not the same size as a dark row in another.",
  ],
  read: [
    "Darker means larger. The faint layer is the first slice of the range, the darkest is the last.",
    "Green (by default) is above zero and red is below. A row that flips between them is crossing its baseline.",
    "Look across rows for the same place on the time axis: a column of dark patches means that many series spiked together.",
    "To read a row's size, look at the band value printed beside its name: one full layer is that much, and the shades stack.",
    "There is no hover in a horizon. The rows are for scanning. Use a line chart for the series that catch your eye.",
  ],
  story: {
    title: "Fourteen servers and a Tuesday",
    paragraphs: [
      "Tomasz was on call for a fleet of fourteen database replicas, which meant that on a bad night he was looking at fourteen CPU graphs and a phone that had not stopped vibrating since two in the morning. The graphs lived in a dashboard with a tall grid of line charts, and the grid needed scrolling.",
      "The thing about scrolling is that you cannot see a pattern across the rows you are not looking at. He kept finding a spike in the fifth graph and then wondering whether the third had spiked at the same moment, scrolling back, and losing the fifth. This is not a skill problem. It is a screen problem.",
      "He wanted all fourteen on one screen, with shared time, and sensitive enough to show that a replica had started drifting a point or two off its own normal. The metric was the deviation of CPU from each replica's 7-day mean, so it had both signs, and zero meant \"as usual\".",
      "He rebuilt the panel as a `<Horizon>` from bui-charts, with one field per replica, three bands, and a 15-minute time axis. The fourteen rows fit in the height that used to hold three line charts, and nobody had to scroll.",
      "The pattern was obvious at once: a thin column of dark red running across rows six to nine at 02:10, two replicas whose CPU had dropped well below normal and two that went up. In a grid of line charts he had read it as four separate incidents. In the horizon it was one: a failover. A leader had gone away and the load had moved.",
      "That changed the postmortem. The team had been chasing four alert rules, and the fix was a single health-check that took 40 seconds to notice the failover. After the check was tightened, pages per failover dropped from 11 to 2, and Tomasz started getting uninterrupted sleep for the first time that quarter.",
      "He is told the horizon chart looks like a barcode that went to art school. He has stopped correcting people.",
    ],
  },
  pitfalls: [
    "**Comparing darkness across rows.** Each series is scaled to its own largest value. A dark row means \"large for this series\", not \"large\". Read the band value beside the name.",
    "**One huge outlier flattens the rest.** The band height comes from the largest absolute value, so a single spike makes every other point fall in the faint layer. Clip or winsorise first if the spike is noise.",
    "**Too many bands.** More than four shades are hard to tell apart. Three is usually the right trade between height and legibility.",
    "**Using it for strictly positive data with no baseline.** It works, but only half the colour scheme is used and the zero is arbitrary. A [time series](/charts/time-series) with a filled area is clearer.",
  ],
  related: [
    { slug: "time-series", why: "When you have a few series and enough height to read the exact values." },
    { slug: "heatmap", why: "When you want to see magnitude as colour over time and value buckets, not folded bands." },
    { slug: "ridgeline", why: "When the rows are distributions of a value instead of series over time." },
  ],
  media: [
    { kind: "youtube", id: "zNIeYVyPaK4", title: "Horizon Chart (Step by Step Tutorial)", channel: "TRUVIZ", note: "Builds a horizon chart layer by layer, which is the quickest way to see how the folding works." },
    { kind: "youtube", id: "LoaZlJAm7SA", title: "Creating Horizon Charts in Tableau - Tutorial & Template workbook", channel: "Marc Reid", note: "Explains the number-of-slices choice and what each shade stands for." },
  ],
}
