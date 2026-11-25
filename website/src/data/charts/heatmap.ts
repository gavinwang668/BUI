import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A grid of cells shaded by count or value: either a distribution over time, or a value for every pair of two categories.",
  intro: [
    "A line chart of average latency is a polite lie. It says the service takes 120 ms, and does not mention that half the requests take 40 and a stubborn few take four seconds. A heatmap keeps the whole population. Time runs along x, the measured value runs up y, and each cell is shaded by how many samples landed in it. Instead of one line you get a landscape, and landscapes have mountains.",
    "Shaded grids are older than computers. In the 1870s the French statistician [Toussaint Loua](https://en.wikipedia.org/wiki/Toussaint_Loua) shaded a table of social statistics for the districts of Paris, one of the early cases of a matrix drawn as colours rather than numbers. Monitoring tools later gave the idea a second life as the picture of latency over time, and now every on-call engineer has stared at one at three in the morning, trying to decide if that orange smear is new.",
    "The chart does two jobs, and it helps to keep them apart. In **sample mode** you hand it raw events and it counts them into buckets. In **cell mode** you hand it a number per pair of categories (day and hour, region and service) and it paints each one. The colour scale is the same in both. The question changes from *where did my samples land?* to *what is the value here?*",
  ],
  math: [
    {
      heading: "Counting samples into buckets",
      body: [
        "Given samples $(x_r, y_r)$, the chart picks a bucket width for each axis. With `Buckets::Auto` it aims for about 20 buckets across x and 10 up y: the raw step is $(\\max - \\min)/(n-1)$, which is then rounded *down* to the nearest nice value (1, 2, 2.5, 5 times a power of ten, and for time axes a ladder of seconds, minutes, hours and days). `Buckets::Count(n)` uses the same rule with your $n$, and `Buckets::Size` uses your width as given.",
        "A sample falls into the bucket whose lower edge is its value rounded down to a multiple of the width $\\Delta$. The cell count is the number of samples in it.",
      ],
      tex: [
        "b(x) = \\Delta_x \\left\\lfloor \\frac{x}{\\Delta_x} \\right\\rfloor, \\qquad N_{ij} = \\#\\{\\, r : b(x_r) = x_i,\\ b(y_r) = y_j \\,\\}",
      ],
    },
    {
      heading: "Log buckets",
      body: [
        "Latencies bunch near zero and trail off for miles. With `y_scale: BucketScale::Log2` or `Log10` the y axis is bucketed in the exponent instead. With base $B$ and a split $q$ per power, the bucket index is the floor of $q \\log_B y$, and the bucket edges are $B^{k/q}$. Non-positive samples cannot be placed on a log axis and are dropped.",
      ],
      tex: ["j(y) = \\left\\lfloor q \\log_B y \\right\\rfloor, \\qquad \\text{edge}_k = B^{k/q}"],
    },
    {
      heading: "From count to colour",
      body: [
        "The colour range runs from `color_min` to `color_max`, which default to the smallest and largest cell in the data. A cell value $N$ becomes a position $t$ in $[0, 1]$, clamped, and the position picks one of $S$ palette steps (64 by default). In opacity mode the same $t$ becomes an alpha, linear or raised to an exponent, over your chosen fill colour.",
        "Cells at or below `hide_le`, or at or above `hide_ge`, are not drawn at all, so near-empty buckets and wash-out outliers can be removed without changing the scale. If the range has zero width, everything gets the top colour.",
      ],
      tex: [
        "t = \\mathrm{clamp}\\!\\left(\\frac{N - c_{\\min}}{c_{\\max} - c_{\\min}},\\,0,\\,1\\right), \\qquad \\text{index} = \\mathrm{round}\\bigl(t\\,(S-1)\\bigr), \\qquad \\alpha = t^{\\gamma}",
      ],
    },
    {
      heading: "Histogram buckets as input",
      body: [
        "[Prometheus](https://prometheus.io/docs/concepts/metric_types/) and [OpenTelemetry](https://opentelemetry.io/) export histograms as cumulative-looking bucket fields with an upper bound (`le`). Set `bucket_layout={BucketLayout::Le}` and the chart reads each field as an ordinal row. The tooltip's `y_histogram` then shows the whole column for the x under your cursor, which is where bimodal distributions give themselves away.",
      ],
    },
  ],
  use: [
    "You want how a **distribution** changes over time, not how a single statistic does: latency, request size, job duration.",
    "You have a value for every pair of two categories, such as day by hour, region by service, or feature by cohort.",
    "Hundreds of thousands of samples would overwhelm a scatter plot. Counting them is cheaper and clearer.",
    "You suspect two populations (fast and slow paths, cache hits and misses) and want to see them as two bands.",
  ],
  avoid: [
    "You want one number's trajectory. [Trend](/charts/trend) or [Time Series](/charts/time-series) will be faster to read.",
    "You need exact values. Colour is a poor ruler. Use a table or turn on `show_value` for small grids.",
    "The data is mostly one value. The scale will paint everything the same shade and call it a day.",
    "States rather than counts, such as up or down. [State Timeline](/charts/state-timeline) merges equal neighbours into readable blocks.",
  ],
  read: [
    "Start with the brightest band, which is where most of the samples are. A line through its middle is roughly the median you would have plotted anyway.",
    "Look at the width of that band. A thin band means consistent behaviour. A fat one means the average is hiding variation.",
    "Then look for second bands and sudden shifts. A new band that appears at a certain time is a new code path, a new customer, or a new bug.",
    "Hover a cell for its value and bucket range. Drag to zoom in time (or in a box, with `SelectionMode::XY`) and use `exemplars` to jump to individual slow requests.",
  ],
  story: {
    title: "The mean was fine. The mean is always fine.",
    paragraphs: [
      "Ravi ran platform reliability for a food delivery app in Hyderabad, where the busiest hour of the day is dinner and the second busiest is also dinner, for a different part of the city. His dashboard had one line for p50, one for p99, and a green status light that had not changed in eleven weeks.",
      "The support queue disagreed with the light. Customers in a few neighbourhoods kept writing that the app felt slow on Friday nights, and the p99 line, when anyone looked at it, did a small shrug at about 900 milliseconds. Fridays, according to the dashboard, were fine.",
      "Ravi's suspicion was that Friday was not slower so much as *different*: a second kind of request that the percentile lines blended into noise. To check that, he needed to see the shape of the whole distribution, minute by minute, for four hours around dinner.",
      "He replaced the two lines with a `<Heatmap>` from bui-charts: raw request durations as samples, one-minute x buckets, base-2 log buckets on y so the long tail got as much room as the bulk, and a Viridis scale with 16 steps so the bands would be easy to count.",
      "On Friday evening the heatmap grew a second band at around 1.6 seconds, thin at 7 pm and thick by 8:30, floating above the main band like a small orange cloud. The p99 line sat in the gap between the two and averaged them out.",
      "Hovering the cells with exemplars turned on led straight to traces, and the traces led to a payment retry that was waiting for a fixed 1.5-second timeout before trying a second provider. The timeout was introduced in a change titled *minor cleanup*.",
      "Shortening it to 300 milliseconds removed the second band entirely, and Friday support tickets about slowness dropped by roughly two thirds. The green status light is still green. Ravi has stopped trusting it and instead trusts the picture of the orange cloud that is no longer there.",
    ],
  },
  pitfalls: [
    "**Colour range drifts.** The scale defaults to the data's own min and max, so two charts of different days are not comparable. Fix `color_min` and `color_max` when they should be.",
    "**Too many steps.** 64 smooth steps look nice and are impossible to compare. Eight to sixteen make bands easy to tell apart.",
    "**Rainbow palettes.** They create edges that are not in the data. Use Viridis, Magma or Cividis for fine differences, and a single-hue scheme for more-is-hotter.",
    "**Bucket size mismatch.** Buckets much smaller than your sample count per bucket give noisy cells. Start with `Buckets::Auto` and only then set a `Size`.",
    "**Log axes drop non-positive values.** Zero and negative samples are silently excluded from log buckets.",
  ],
  related: [
    { slug: "state-timeline", why: "When each moment has a state, not a count, and equal neighbours should merge." },
    { slug: "histogram", why: "The distribution at a single moment, with each bucket drawn as a bar." },
    { slug: "ridgeline", why: "Several distributions stacked with overlap, when you have a handful of groups instead of a time axis." },
    { slug: "spectrogram", why: "A heatmap of signal strength over frequency and time." },
  ],
  media: [
    { kind: "youtube", id: "J7cd1-g1O7A", title: "Seaborn Heatmap - How to Visualise Correlations and Data With Heatmaps in Python", channel: "Andy McDonald", note: "Practical heatmap tutorial for correlation matrices, using Python's seaborn." },
  ],
}
