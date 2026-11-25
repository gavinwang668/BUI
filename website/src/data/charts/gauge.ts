import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A value on a dial, between a minimum and a maximum, with the limits marked on the rim.",
  intro: [
    "The gauge is the oldest chart in the building that still works. Long before dashboards, a needle on a dial told you the boiler pressure, and the part that mattered was not the number. It was the red zone painted near the end. You did not need to read anything to know you were close to a bad afternoon.",
    "That is the idea the gauge chart keeps: a value is not just a quantity, it is a position in a range, and some parts of the range are worse than others. [Grafana's Gauge panel](https://grafana.com/docs/grafana/latest/panels-visualizations/visualizations/gauge/), which this chart ports, replaced the needle with a bar that fills along an arc and puts a thin ring of threshold colours on the outside. You see how full it is, and how far to the next colour.",
    "It also has the faults of its ancestors. A dial spends a lot of ink on one number, which is why dashboards that are all gauges look like the cockpit of an aircraft designed by a committee. Used once or twice, where the range is real, it is excellent. Used twenty times, it is wallpaper.",
  ],
  math: [
    {
      heading: "From value to angle",
      body: [
        "Given a value $v$ and a range $[a, b]$ (`min` and `max`, default 0 and 100), the fraction of the arc to fill is the clamped position of $v$ in the range. The default shape is not a full circle: the arc starts at 250 degrees and sweeps 220 degrees clockwise, leaving a gap at the bottom. `GaugeShape::Circle` sweeps the full 360.",
        "Here $\\theta$ is the total sweep (220 or 360). The filled part is an arc of length $f\\theta$ starting at the start of the track.",
      ],
      tex: [
        "f = \\operatorname{clamp}\\!\\left(\\frac{v - a}{b - a},\\,0,\\,1\\right), \\qquad \\text{filled angle} = f\\,\\theta",
      ],
    },
    {
      heading: "Neutral point",
      body: [
        "With a `neutral` value $n$ the bar does not grow from the start of the arc but from $n$. Let $f_n$ be the fraction of $n$ computed the same way. The bar then covers the angles between the two fractions, in whichever direction the value lies.",
        "This is what makes the gauge usable for balances, temperature differences and anything that can be above or below a reference.",
      ],
      tex: [
        "\\text{start} = \\min(f, f_n)\\,\\theta, \\qquad \\text{length} = |f - f_n|\\,\\theta",
      ],
    },
    {
      heading: "Geometry and segments",
      body: [
        "The radius is whatever fits: the largest circle that fits the width, or, for the open arc, the height measured from the top to the lowest point of the arc. The bar is $w = \\text{bar\\_width\\_factor} \\times r/3$ wide (never under 2 px), so the default factor of 0.5 gives a bar about a sixth of the radius thick. Threshold labels, markers and glows take room from the inside before the final radius is fixed.",
        "With `segment_count` $= m$, the arc is cut into $m$ pieces. A segment is never drawn under 3 px plus its gap, so the count is reduced if the arc is too short to hold them. Each lit segment is coloured by the value its own position stands for, so thresholds show up as bands in the bar.",
        "The colour of the whole bar comes from a matching value mapping first, otherwise from `color`, which by default is the threshold step the value has reached.",
      ],
    },
  ],
  use: [
    "The value has a real, known range with named limits: a temperature against its rating, disk usage against capacity, battery from 0 to 100.",
    "You want a status that reads at a distance. The ring of threshold colours tells people how close the next step is without a single label.",
    "Values can be on either side of a reference, such as a balance or a deviation. Set `neutral` and the bar grows from there.",
    "You have a handful of these, a few per panel, each representing a different resource.",
  ],
  avoid: [
    "The range is made up. A gauge from 0 to 1000 for a metric that sits between 3 and 7 is a very large dial with a very small needle.",
    "The history matters more than the current value. The built-in sparkline helps, but [Time Series](/charts/time-series) is the chart for trends.",
    "You have dozens of values to rank. Arcs are hard to compare precisely. Use [Bar Gauge](/charts/bar-gauge), where lengths line up on a common baseline.",
    "You only need the number. A [Stat](/charts/stat) tile takes less room and shows it bigger.",
  ],
  read: [
    "Start with the number in the middle: the gauge writes it large and fits it to the dial.",
    "Look at the bar's colour and how far it runs. Its end position against the arc tells you the share of the range used.",
    "Check the thin ring outside the bar. It shows each threshold step in its own colour, so you can see how much room is left before the next one.",
    "If threshold labels are on, they give the limits as numbers. The small sparkline below the value, when present, shows where the value has been.",
  ],
  story: {
    title: "A tank, a dial, a Sunday",
    paragraphs: [
      "Tobias looked after the cooling system of a small brewery, which in practice meant a laptop, a pager and four fermentation tanks that each held a month of somebody's work and had no sense of humour. His old monitoring page was a table of numbers. It was accurate and nobody read it.",
      "The yeast cares about one thing above all others: temperature. Between 18 and 21 degrees the beer is happy. Above 24 it develops opinions, and below 14 it goes to sleep. Tobias needed the brewers, none of whom wanted to read a table, to see on a glance whether each tank was inside its band.",
      "A line chart was no use for that. It would show the history but make you work out whether the latest point was good. What the brewers needed was a dial for each tank, with the good zone drawn on it, and a clear signal when the needle drifted toward the end.",
      "He wired the sensor feed into a small desktop app built on BUI, and put one `<Gauge>` over a frame with a field per tank. He set `min` to 10 and `max` to 30, thresholds at 14, 18, 21 and 24, and turned on `show_threshold_labels` so the numbers were printed on the rim. A fifth tank, which was only ever a cold store, got `neutral` set at its target so its bar grew in both directions around it.",
      "On the first Sunday after it went up, tank three crept past 22 while Tobias was out. Nobody had to read anything. A brewer walking past saw a bar nudging into the amber band and a ring that was nearly red, and turned the chiller up. The tank peaked at 23.4.",
      "Over that season the brewery lost no batches to temperature, against two the year before, one of which had been given a name by the staff. The table of numbers is still there, in a tab nobody opens.",
      "Tobias says the best review he got was a brewer asking whether the gauges could be made bigger, which in monitoring is the equivalent of applause.",
    ],
  },
  pitfalls: [
    "**Range is part of the message.** The same reading looks fine on 0 to 1000 and alarming on 0 to 10. Set `min` and `max` to real limits, not to whatever makes the bar look good.",
    "**Thresholds in the wrong order.** The colour is the step the value has reached, so bad-at-the-low-end metrics (battery, free space) need their steps reversed with the bad colour first.",
    "**Clamping.** A value above `max` fills the arc completely and no further. The printed number is still the true one, but the bar cannot tell you how far over you are.",
    "**Too many segments.** Past about 20, segments get thinner than the 3 px floor and the chart silently shows fewer of them than you asked for.",
    "**Glows on busy pages.** `bar_glow` and `center_glow` look good on a dark wall display and noisy in a grid of twelve.",
  ],
  related: [
    { slug: "stat", why: "When you only need the number, large, with a sparkline." },
    { slug: "bar-gauge", why: "When you want several values compared on one scale with exact lengths." },
    { slug: "bullet", why: "When you need a target marker and qualitative bands in a compact strip." },
  ],
  media: [
    { kind: "youtube", id: "QwXj3y_YpnE", title: "Beginners guide - Visualizing Gauges | Grafana", channel: "Grafana", note: "Official Grafana walkthrough of the gauge panel and thresholds." },
  ],
}
