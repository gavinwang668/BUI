import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A compact bar showing one measure against a target and three performance bands.",
  intro: [
    "Dashboards used to be full of gauges: round dials with a needle, a red zone, a green zone, and a quiet sense that someone at the software company had once seen a car. A gauge uses a large square of screen to show one number. Put six of them side by side and you have used the whole page, learned six numbers, and still cannot tell which is worst.",
    "Stephen Few designed the [bullet graph](https://en.wikipedia.org/wiki/Bullet_graph) in the mid-2000s as a replacement, and the design is very sensible. A single bar shows the value. A short tick across it marks the target. Behind both, three bands of grey, from dark to light, mark poor, fair and satisfactory ranges. All of that fits in one thin horizontal strip, so you can stack ten of them and compare every KPI in one glance.",
    "The name is a joke the designer never intended. The thick bar and the small tick resemble a bullet, which is the only violent thing about it. Everything else is a calm summary of how things are going.",
  ],
  math: [
    {
      heading: "One row, five marks",
      body: [
        "Each row has a name, a measured value $v$, an optional target $g$, and optional three band edges $b_1 \\le b_2 \\le b_3$ given as an array in that order, smallest first. They are the upper ends of the poor, fair and satisfactory ranges.",
        "The bands are bars drawn from the baseline, widest first, one on top of the other. The largest band, ending at $b_3$, is the lightest and sits at the back. The next, ending at $b_2$, is darker, and the poor band, ending at $b_1$, is the darkest and drawn in front. So the visible stripe of each colour is the *difference* between neighbouring edges, though the data holds cumulative values.",
      ],
      tex: ["\\text{poor} = [0, b_1], \\quad \\text{fair} = (b_1, b_2], \\quad \\text{satisfactory} = (b_2, b_3]"],
    },
    {
      heading: "The value bar and the target tick",
      body: [
        "The value is a bar from the baseline to $v$, in the series colour, at about 30% of the slot thickness: narrower than the bands, so the bands read as a background. The target is a thin tick across the row at $g$. Its thickness is a fixed fraction of the data range, which is $\\varepsilon = 0.006\\,M$ where $M$ is the largest finite value among the measures, targets and top band edges in the whole chart.",
        "Then the tick is a small bar of height $2\\varepsilon$ whose lower end is $g - \\varepsilon$, centred on the target. The value axis defaults to a minimum of zero, since the value is a bar and bars must start there. With no `ranges`, the bands are not drawn, and with no `target` the tick is omitted.",
      ],
      tex: ["\\varepsilon = 0.006\\,M, \\qquad M = \\max\\{\\, v_i,\\ g_i,\\ b_{3,i} \\,\\}, \\qquad \\text{tick} = [\\,g - \\varepsilon,\\ g + \\varepsilon\\,]"],
    },
  ],
  use: [
    "You report several KPIs together and each has a target and a sense of good and bad: sales against quota, uptime against SLA, defects against tolerance.",
    "You want a dashboard that is dense but still readable, with a dozen measures in one panel.",
    "The audience wants to know not just whether a target was hit but how comfortably, which the bands show.",
  ],
  avoid: [
    "There is no target and no sense of a good range. Then a plain bar or a [lollipop](/charts/lollipop) does the job with less decoration.",
    "The measures are on very different scales in the same chart. Each row shares a single axis, so a metric in the thousands will flatten one in single digits. Chart them separately, or normalise to a percentage of target.",
    "You need to show history. A bullet is a snapshot. Use a [stat](/charts/stat) with a sparkline or a [time series](/charts/time-series).",
    "A single value, shown large, is the whole message. Use [gauge](/charts/gauge) or [stat](/charts/stat).",
  ],
  read: [
    "Find the thick coloured bar. Its end is the actual value.",
    "Find the thin tick across the row. If the bar passes it, the target is met, if the bar falls short, the gap is visible.",
    "Look at the grey bands behind. Dark means poor, medium means fair, light means satisfactory, so the shade the bar ends in tells you how good the result is.",
    "Compare rows by their shape. Rows share one axis, so a longer bar really is a bigger number.",
    "Hover a row for the value, the target and the band edges.",
  ],
  story: {
    title: "Six dials, one red number",
    paragraphs: [
      "Nkechi ran customer support for a payments company, and her Monday dashboard had six gauges. They were large, glossy and round, and she had inherited them from a predecessor who had clearly loved speedometers. Each showed one number: first response time, resolution rate, backlog, CSAT, escalations and a thing called Happy Index which nobody could define.",
      "The trouble began when the vice-president asked which of the six needed attention. Nkechi looked at the dials. Three were in the green. Two were in what might be described as olive. One was a needle at an angle that could be read either as bad or as the artist's choice.",
      "Gauges carry a surprising amount of ceremony for the information they provide. You read the needle, find the zone, remember what the zone means, and then try to compare it with five other needles, none of which are drawn to the same scale of dread.",
      "She rebuilt the panel with `<Bullet>`, one row per metric, with the value, the quarterly target, and three bands that her team had agreed on years ago and never drawn. The six rows fitted in one third of the space the gauges had used.",
      "Now it was quick. The bars for response time and CSAT ended in the lightest band, well past their ticks. Backlog was a bar that stopped inside the dark band, a long way short of its tick. Escalations, which had looked fine as a needle, was in the fair band and drifting toward the dark one.",
      "The vice-president pointed at the backlog row before anyone spoke. The team added two agents on the late shift and, within five weeks, backlog moved into the fair band and its bar reached the target tick by the end of the quarter. The Happy Index was retired, quietly, and without a funeral.",
      "A gauge shows you a number. A bullet shows you a number, the goal, and whether you ought to worry.",
    ],
  },
  pitfalls: [
    "**Band order.** Pass `ranges` as `[poor, fair, satisfactory]` upper ends, smallest first. The bands are drawn as nested bars, so passing them out of order hides one of them.",
    "**Higher is not always better.** Bands are shaded dark for the lowest range, which assumes low is poor. For error rates or latency, reverse the meaning in your head or in a label.",
    "**A zero baseline is assumed.** The value axis starts at zero by default, and the bars are anchored there. Setting a higher minimum cuts the bars and misleads.",
    "**Target tick scale.** The tick is sized from the largest number in the whole chart, so one very large row makes every tick thinner on the others.",
  ],
  related: [
    { slug: "gauge", why: "For one value on an arc, when the circular form is wanted and the space is available." },
    { slug: "bar-gauge", why: "For how full each value is between a minimum and a maximum, with thresholds." },
    { slug: "stat", why: "When one big number with a trend and a change is the message." },
  ],
  media: [
    { kind: "youtube", id: "Blrn53FyJ3o", title: "Tutorial: Stephen Few-style bullet chart in Power BI using core visuals!", channel: "Christine Payton", note: "Builds a bullet chart in the style Stephen Few designed, using Power BI rather than this library." },
    { kind: "youtube", id: "IS6nU4rx_yE", title: "Create Professional Actual vs Target Variance Charts - Bullet Charts", channel: "Aethenos", note: "Shows how bullet charts compare actual to target." },
  ],
}
