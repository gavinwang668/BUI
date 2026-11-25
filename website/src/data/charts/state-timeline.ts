import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "One lane per thing, with coloured blocks showing which state it was in and for how long.",
  intro: [
    "Some data is not a number. A service is *healthy* or *degraded* or *down*. A machine is *idle*, *running* or *faulted*. A feature flag is on or it is off, and the interesting question is when it changed and what else changed at the same time. Plotting those on a line chart means inventing numbers (healthy is 1, degraded is 2) and then trying to read an axis you made up.",
    "A state timeline drops the axis and keeps the idea. Time runs left to right, each lane is one thing, and a block of colour says *in this state from here to there*. It looks like a [Gantt chart](https://en.wikipedia.org/wiki/Gantt_chart) and behaves like a very patient historian. It came into operations tooling because it is the cheapest way to see correlation: if three lanes turn red at the same x, you have a shared cause, and a shared cause is a shorter meeting.",
    "In bui-charts the blocks are made from **samples**. Each row is a value at a time. Neighbouring samples with the same state are merged into one block. That means a service that reports *OK* every ten seconds for a day is one long green bar, and not 8,640 green dots that the reader has to take on trust.",
  ],
  math: [
    {
      heading: "Samples to segments",
      body: [
        "A lane is a sequence of samples $(t_i, s_i)$ with times $t_i$ and states $s_i$. A state is the value after it has been passed through the lane's colouring rule: a value mapping, a threshold step, or a colour scheme. Two samples are *the same state* when their resolved keys are equal, not when their raw numbers are: with thresholds, a CPU at 31% and one at 44% are both *normal* and merge.",
        "A segment opens at the first sample of a state and closes at the next sample with a different state, or at the end of the data. Writing $K(s)$ for the resolved key:",
      ],
      tex: [
        "\\text{segment}_m = \\bigl[\\,t_a,\\ t_b\\,\\bigr), \\qquad K(s_a) = K(s_{a+1}) = \\cdots = K(s_{b-1}) \\neq K(s_b)",
      ],
    },
    {
      heading: "Gaps and missing values",
      body: [
        "Two things end a segment early. First, a missing value ($s_i$ is null): by default the block ends at that sample and a gap follows, because *unknown* is not the same as *same as before*. `span_nulls` can bridge nulls always, or only below a given gap length. Second, `insert_nulls` is a timeout: if the next sample arrives more than that long after the previous one, the block is closed at the previous sample plus the timeout, so a source that went silent does not appear to stay healthy forever.",
        "With `merge_values={false}` nothing is merged and every sample becomes its own block, which is the right choice when each sample is an event rather than a reading.",
      ],
      tex: [
        "\\text{end} = t_{i-1} + \\tau \\quad \\text{if } t_i - t_{i-1} > \\tau",
      ],
    },
    {
      heading: "Blocks to pixels",
      body: [
        "The x scale maps the data's time range linearly onto the width of the plot, so a block's width is exactly proportional to its duration. Lanes stack vertically, each of height `row_height`, and are paginated by `per_page`. A block's label is written only if it fits, and `align_value` puts it left, centre or right.",
      ],
    },
  ],
  use: [
    "The data is **categorical over time**: up/down, running/idle, 200/404/500, on/off.",
    "You have a number that is really a level, such as CPU in normal, high and critical bands, and the thresholds are what matters.",
    "You want to compare many lanes at once and look for things that happen together.",
    "How long each state lasted matters, not just how often it happened.",
  ],
  avoid: [
    "Each sample is a separate result and every one matters, such as a nightly job. [Status History](/charts/status-history) draws one cell per sample.",
    "You care about the shape of a numeric signal. Colour bands throw the curve away. Use [Trend](/charts/trend).",
    "There are more than about eight states. Colours stop being distinguishable and the legend becomes homework.",
    "You need exact durations. Block widths are honest but approximate. Put the numbers in a table.",
  ],
  read: [
    "Scan each lane for its dominant colour. That is the normal state. Everything else is an exception.",
    "Look for vertical alignment across lanes. Blocks that change at the same x are usually one event seen from several places.",
    "Note the short blocks. A thin sliver of red that repeats is a flapping service, which is often worse than a long outage.",
    "Hover for the state, its start and end, and the value. `Tooltip::multi()` shows every lane at the same moment. Drag to zoom in time.",
  ],
  story: {
    title: "Seven red slivers",
    paragraphs: [
      "Hanna was the on-call engineer for a ticketing company in Tallinn on the night the big concert went on sale. Pages kept firing for the checkout service, then stopping, then firing again, in a rhythm that a colleague described as someone knocking politely on a very large door.",
      "Each page, taken alone, looked like a blip. The status line on the dashboard showed *degraded* for ninety seconds and then *OK*. Nobody could say whether this was one problem repeated or seven different ones, which matters, because one problem has one fix and seven problems have a long night.",
      "What Hanna needed was to see the state of the checkout service, its database, its payment gateway and the cache layer on the same time axis, for the last six hours, with the state names spelled out. She did not want numbers. She wanted to know who was red when.",
      "She built it with `<StateTimeline>` from bui-charts: one lane per component, a shared value mapping from *OK* to green, *DEGRADED* to amber and *DOWN* to red, and a lower row height so all eleven lanes fit without a scroll bar.",
      "The picture was unambiguous. Seven red slivers on the cache lane, each about forty seconds wide, and immediately after each one, an amber block on checkout. The database lane was green throughout. It was never the database. It was never the database, in the way that it is never the database right up until it is.",
      "The cache nodes were being recycled by an autoscaler that treated a brief memory spike as a reason to restart. Hanna raised the threshold and the slivers disappeared. The concert sold out with checkout at 100 per cent for the last four hours.",
      "She printed the chart and stuck it on the team's fridge with a magnet that said *it is the cache*. Nobody has taken it down, and nobody has said it was not.",
    ],
  },
  pitfalls: [
    "**One mapping per lane.** If every lane builds its own value mapping, the legend lists *OK* once per lane. Share one `static` mapping.",
    "**Null means unknown.** A missing sample leaves a gap by default. If it means no change, set `span_nulls`. If the source stops sending, set `insert_nulls`.",
    "**Merging hides samples.** With the default merge, ten identical readings are one block. Turn merging off when every sample is an event.",
    "**Too many lanes.** Past a screenful, use `per_page` and keep the number at what your panel can show.",
    "**Colour as the only signal.** Value text inside the block helps readers who cannot tell amber from green.",
  ],
  related: [
    { slug: "status-history", why: "When each sample is a result and neighbours should stay as separate cells." },
    { slug: "gantt", why: "When blocks are planned tasks with dependencies rather than observed states." },
    { slug: "heatmap", why: "When you have counts or distributions instead of discrete states." },
    { slug: "trend", why: "When the number's shape matters more than the band it is in." },
  ],
  media: [
    { kind: "youtube", id: "a9wZHM0mdxo", title: "Beginners Guide - How to Configure a State Timeline Panel | Grafana", channel: "Grafana", note: "Official Grafana beginner guide to configuring a state timeline panel, the chart this one is modelled on." },
  ],
}
