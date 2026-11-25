import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A grid with one cell per sample and one row per thing, coloured by the result of each check.",
  intro: [
    "Some things happen on a schedule, and the schedule is the point. A nightly backup. A health check every minute. Daily sales. Each run has a result: passed, failed, slow, or a number worth colouring. The useful picture is not the length of a state but the **row of results**, like a row of stitches, where a single wrong one is visible from across the room.",
    "That is the Status History chart. Rows are things, columns are samples, and each cell is coloured by what that sample said. It is cousin to the [State Timeline](/charts/state-timeline), with one stubborn difference: it never merges. Ten green checks in a row are ten green cells, and that is the information. You can count them. You can notice the fourth one was amber.",
    "This is the chart of the calendar wall, the habit tracker, the CI matrix. Humans are very good at scanning a grid for the odd one out, and very bad at reading ten thousand log lines for the same thing. The chart takes advantage of that, and has the manners not to talk much.",
  ],
  math: [
    {
      heading: "One cell per sample",
      body: [
        "For row $r$ you supply samples $(x_i, s_i)$. The chart draws one cell for each sample, centred on $x_i$, in the colour that the lane's rule assigns to $s_i$. That rule is a value mapping (text or code to named colour), a threshold step, or a colour scheme over a numeric range. Nothing is merged: the cell is the sample.",
        "The cell width is the column width in pixels, `col_width`, which by default leaves a small gap between neighbours. Setting it to 1 removes the gap and makes the grid read as a continuous strip. A smaller value gives separated dots. `row_height` works the same way vertically.",
      ],
    },
    {
      heading: "Numbers as colour",
      body: [
        "For numeric samples with the default scheme, the colour position is the value's place in the range given by `min` and `max`, clamped. With thresholds, the colour is the step the value falls in, so 31 and 44 on the same band show the same colour. With a value mapping, each distinct value has its own named colour and appears once in the legend.",
        "Rules can differ by row: `color`, `thresholds` or `mappings` set on an individual `<StatusRow>` override the chart's defaults.",
      ],
      tex: [
        "t = \\mathrm{clamp}\\!\\left(\\frac{v - v_{\\min}}{v_{\\max} - v_{\\min}},\\ 0,\\ 1\\right)",
      ],
    },
    {
      heading: "x is a number, not just a time",
      body: [
        "The columns are positioned by x, which does not have to be a timestamp. It can be a day number, a build number, a test run. When x is time, the axis uses clock ticks. Otherwise give `x_axis` a `format` to turn numbers into labels. A weekly calendar grid is just rows of weekdays with x as the week.",
      ],
    },
  ],
  use: [
    "Each sample is a **result**: a check passed or failed, a job succeeded, a day's total was in or out of range.",
    "You want to scan many rows for the odd cell, such as one failing job among fifty nights.",
    "The *count* of ok and not ok matters, not just the lengths of runs.",
    "The x axis is not really time: build numbers, test runs, weeks.",
  ],
  avoid: [
    "States that last until they change, such as up or down for hours. A [State Timeline](/charts/state-timeline) merges equal neighbours into readable blocks.",
    "The shape of a number matters. Colour bins throw it away. Use [Trend](/charts/trend).",
    "You have tens of thousands of samples per row. Cells shrink below a pixel. Aggregate first, or use a [Heatmap](/charts/heatmap).",
    "You need precise values from the picture. Colour is a verdict, not a ruler.",
  ],
  read: [
    "Scan across a row for its normal colour. The exceptions are what you came for.",
    "Scan down a column. A vertical stripe of the same bad colour means many things failed at the same sample, which is one cause, not twenty.",
    "Look for patterns in the exceptions: every Sunday, every fifth run, always the same row.",
    "Hover a cell for its value and time. `Tooltip::multi()` compares every row at that sample.",
  ],
  story: {
    title: "The Sunday that kept failing",
    paragraphs: [
      "Ayodele was the only person on the platform team at a small online pharmacy in Lagos who liked checking things, which in practice made him the person who was asked about all of them. There were thirty-four scheduled jobs: stock sync, invoice generation, delivery manifests, and a nightly reconciliation job that everybody agreed was important and nobody could explain.",
      "His monitoring was an email. One, per failure, to a shared inbox that had accumulated 3,100 unread messages. Occasionally somebody would search it for a job name and report, with the energy of a person finding a coin in a coat, that it had indeed failed on a Thursday in May.",
      "The question that mattered was not whether a job failed but whether it failed *often* and whether it failed *together* with other jobs. Email is excellent at the first kind of fact and spectacularly poor at the second.",
      "He loaded ninety days of results into `<StatusHistory>` from bui-charts: one row per job, one cell per run, a shared mapping from *success* to green, *retry* to amber and *failed* to red, and `show_value` turned off since the colours were enough. Thirty-four rows by ninety columns fit in a quarter of a screen.",
      "The grid was mostly green with a pattern that no email could have shown him: a faint red column every seventh cell, always on a Sunday, running down through eleven jobs at once. It was not eleven problems. It was one: a database snapshot that locked the tables for about six minutes starting at 02:00.",
      "Moving the snapshot to 04:00 took one line in a cron file and made the Sunday column vanish. The failure count over the next ninety days dropped from 142 to 19, and the 19 were genuinely different from one another, which is how he knew they were real.",
      "The shared inbox was archived. Nobody noticed, which is the best review a monitoring system can get.",
    ],
  },
  pitfalls: [
    "**Merging is not on offer.** Every sample is a cell. If you have long runs of one state, use a State Timeline instead.",
    "**One mapping per row.** Share a single `static` mapping across rows so the legend lists each state once.",
    "**Colour alone.** A [colour-blind](https://en.wikipedia.org/wiki/Color_blindness) reader cannot separate amber from green. Show text for small grids, or choose distinguishable colours.",
    "**Gaps look like data.** Irregular sampling leaves uneven spacing. Decide whether x is a time or just a count of runs.",
    "**Too many rows.** Use `per_page` and a number that fits your panel.",
  ],
  related: [
    { slug: "state-timeline", why: "When states last until they change and neighbours should merge into blocks." },
    { slug: "heatmap", why: "When cells hold counts or values on a continuous colour scale, and the grid is large." },
    { slug: "waffle", why: "When you want a grid of units to count, rather than a history." },
    { slug: "gantt", why: "When the rows are planned tasks with durations and dependencies." },
  ],
  media: [
    { kind: "youtube", id: "a9wZHM0mdxo", title: "Beginners Guide - How to Configure a State Timeline Panel | Grafana", channel: "Grafana", note: "Grafana's guide to the state timeline panel, the closest sibling of status history (same family of state-over-time panels)." },
  ],
}
