import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Trend with a clock: lines, areas and bars over time, with annotations, timezones and yesterday drawn underneath.",
  intro: [
    "If you have ever been paged, you have looked at a time series. A line, wandering across hours, and a sinking feeling at one particular place on it. Every operational question eventually becomes one of three: *when did it start?*, *what else happened then?* and *is this normal for a Tuesday?* A time series chart exists to answer those without making you open another tab.",
    "Time is a strange x axis. Hours have sixty minutes, days have twenty-four hours, except twice a year when they do not, and nobody agrees where midnight is. So this chart takes everything [Trend](/charts/trend) can do (stacking, thresholds, two axes, smooth lines) and adds the time-specific parts: ticks that land on round clock times, labels in the timezones your team lives in, and a way to overlay the same series from a day or a week ago.",
    "The first time series charts predate computers by a long way. [William Playfair](https://en.wikipedia.org/wiki/William_Playfair) was drawing economic data against years in the 1780s, and the format has been getting more decorated ever since. The modern version is mostly an argument about how many annotations you can add before the line disappears. The answer, for the record, is fewer than you think.",
  ],
  math: [
    {
      heading: "Time as a number",
      body: [
        "Internally, x is a number of milliseconds since the [Unix epoch](https://en.wikipedia.org/wiki/Unix_time). Everything the chart does with x is linear: the visible range $[t_0, t_1]$ is mapped onto the plot width $W$ by $p = W\\,(t - t_0)/(t_1 - t_0)$. Timezones change only what the *labels* say, never where points are drawn.",
        "That separation is why you can show the same chart in two zones and not have the lines move.",
      ],
      tex: ["p(t) = W\\,\\frac{t - t_0}{t_1 - t_0}"],
    },
    {
      heading: "Choosing tick marks",
      body: [
        "A number axis picks nice steps from 1, 2, 2.5, 5, 10. A time axis cannot, because a nice step for time is 15 minutes, not 12.5. The chart keeps a fixed ladder of 37 steps from one millisecond to years (1, 2, 5, 10, 20, 50, 100, 200, 500 ms, then seconds, minutes, hours, days and so on) and picks the smallest step that is at least the span divided by the number of labels that fit.",
        "The first tick is the first multiple of the step at or after $t_0$ in **local** time, which is to say after shifting by the zone's UTC offset $\\delta$. That is how ticks land on midnight and on the hour in the viewer's zone, not in UTC.",
      ],
      tex: [
        "s = \\min\\{\\, s' \\in \\mathcal{S} : s' \\ge (t_1 - t_0)/n \\,\\}, \\qquad t_{\\text{first}} = \\left\\lceil \\frac{t_0 + \\delta}{s} \\right\\rceil s - \\delta",
      ],
    },
    {
      heading: "Time compare",
      body: [
        "`time_compare` draws every series a second time, shifted forward by a fixed amount $\\Delta$ (24 hours for yesterday, 7 days for last week). The shifted copy at time $t$ is the original value at $t - \\Delta$. The copies are dashed and styled like their source so they read as the same measure at another time.",
        "Your data has to cover the earlier period as well. To compare the last 24 hours with the 24 before, pass 48 hours of rows.",
      ],
      tex: ["y^{\\text{compare}}(t) = y(t - \\Delta)"],
    },
    {
      heading: "Stacking and percent",
      body: [
        "Stacked series accumulate positive and negative values separately, and `Stacking::PERCENT` divides each value by the sum of the absolute finite values at that time. The details match [Trend](/charts/trend) exactly, because this chart is Trend with a time axis.",
      ],
    },
  ],
  use: [
    "Values change over **time** and you want to see when: metrics, latency percentiles, request rates, temperatures.",
    "You need to read a spike against events. Annotations mark deploys, incidents and maintenance on the chart itself.",
    "The question is whether today is unusual. Time compare puts yesterday or last week underneath.",
    "Several teams in several timezones look at the same chart.",
    "You want to link an aggregate to individual samples: exemplars connect a percentile line to the slow requests behind it.",
  ],
  avoid: [
    "x is not time. [Trend](/charts/trend) takes any ordered numeric x.",
    "Each time slot has a range, not one value. [Candlestick](/charts/candlestick) keeps open, high, low and close.",
    "You want a whole distribution per time slot. A line of p99 hides the shape. Use a [Heatmap](/charts/heatmap).",
    "The data is a state that holds until it changes, across many things. [State Timeline](/charts/state-timeline) merges it into readable blocks.",
  ],
  read: [
    "Check the time range and the zone first. A chart that is reading UTC when you think in local time can make a deploy look like it happened at the wrong end of the day.",
    "Find the change and then the cause: annotations and aligned series often explain it.",
    "If time compare is on, look at where the dashed copy and the solid line part ways. Agreement means normal. Divergence is what you are here for.",
    "Hover for a shared tooltip with every series at one moment. Drag to zoom, double-click to reset.",
  ],
  story: {
    title: "Tuesday looks different from Monday",
    paragraphs: [
      "Priya worked on the on-call rota for a payments company with engineers in Pune, Lisbon and Austin. Every incident review began with the same sentence, in three accents: *what time was that, in what zone?* Roughly a third of the review was spent on arithmetic.",
      "A graph of error rate for the checkout flow showed a bump on a Tuesday afternoon. It was small. Was it an incident? The line was a little above where it had been the previous few hours, but not above where it had been last Wednesday, and nobody could remember what a normal Tuesday looked like.",
      "Priya wanted one chart that made three things obvious: the zones the team lived in, the deploys that had gone out that day, and what the same hours had looked like exactly a week earlier. She did not want a second dashboard. She had a rule about dashboards that involved a pause and a sigh.",
      "She built the panel with `<TimeSeries>` from bui-charts: the error-rate series with a one-week `time_compare`, an annotation for each deploy pulled from the release log, and the three timezones on the axis so the labels read in Pune, Lisbon and Austin time at once.",
      "The week-before line lay right on top of the bump. Every Tuesday at about 14:10 Lisbon time, the error rate climbed by about 0.4 percentage points as a batch job on the partner side retried a pile of failed charges. The deploys were not near it. The bump was not an incident. It was a habit.",
      "That one chart saved the review. It also showed, in the same view, an actual incident two hours later, where the solid line left the dashed one and kept going up, right after the 16:30 deploy marker. The review took eleven minutes and nobody did any arithmetic.",
      "The partner has since fixed their retry job. The dashed line and the solid line now lie on top of each other all week, which Priya describes as the most boring and most beautiful thing she owns.",
    ],
  },
  pitfalls: [
    "**Wrong zone in the labels.** Positions never change with the zone, only the text. Always check which zone the axis claims.",
    "**Time compare needs data.** If you pass 24 hours and ask to compare with 24 hours ago, the dashed line has nothing to show.",
    "**Gaps.** By default a missing value breaks the line, which is usually the truth. `span_nulls` bridges them. Use it only when missing means skipped.",
    "**Too many annotations.** Fifty markers turn the chart into a barcode. Show the ones that explain something.",
    "**Averaging away the spike.** Rate and average series smooth short bursts. Show a max or a percentile too.",
  ],
  related: [
    { slug: "trend", why: "The same chart for any ordered numeric x, such as distance or speed." },
    { slug: "heatmap", why: "When you want the whole distribution at each time slot." },
    { slug: "state-timeline", why: "When the data is categorical states rather than numbers." },
    { slug: "horizon", why: "When you have dozens of series and each needs a thin strip." },
  ],
  media: [
    { kind: "youtube", id: "RKtW87cPxsw", title: "Deep Dive - Time Series Panel Visualizations: What Are They? How to Get Started? | Grafana", channel: "Grafana", note: "Official Grafana deep dive into the time series panel, which this chart is modelled on." },
  ],
}
