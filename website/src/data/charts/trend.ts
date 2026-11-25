import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Lines, areas, bars and steps over an ordered numeric x, with stacking, thresholds and two axes.",
  intro: [
    "A trend chart is what most people picture when somebody says *graph*: a number on the left, an x axis along the bottom, a line wandering across. It is also the chart most likely to be used for the wrong thing, because a line is so persuasive. The moment two dots are joined, the reader believes something happened between them.",
    "The form is old and has a known father. [William Playfair](https://en.wikipedia.org/wiki/William_Playfair) drew the first line charts of economic data in the 1780s, showing England's trade balance as a pair of lines with the gap between them shaded. His point was that a picture of a number moving is worth a table of the number standing still. Two centuries later the point stands, and we still shade the gap.",
    "Trend in bui-charts is the general version. Its x is any numeric value that **only goes up**: speed, distance, depth, load. If x is a clock, [Time Series](/charts/time-series) is the same chart with the right axis. Each series can be a line, an area, bars or points, stack with its neighbours, sit on its own axis and change colour when it crosses a limit. It is a lot of chart. Use less of it than you can.",
  ],
  math: [
    {
      heading: "Stacking",
      body: [
        "Series that share a stacking group are drawn on top of each other. At each x index $i$, series $j$ in the group has a raw value $v_{ij}$. Positive and negative values accumulate in separate piles, so a loss never eats into a gain. The base of a series is the current pile, and its plotted top is the pile plus its own value.",
        "Missing values (not finite) are skipped: they add nothing to the pile and leave a break in that series' line. Hidden series and series with a `fill_below_to` floor never join the stack.",
      ],
      tex: [
        "\\text{top}_{ij} = \\text{base}_{ij} + v_{ij}, \\qquad \\text{base}_{ij} = \\sum_{k<j,\\ \\mathrm{sign}(v_{ik}) = \\mathrm{sign}(v_{ij})} v_{ik}",
      ],
    },
    {
      heading: "Percent stacking",
      body: [
        "With `Stacking::PERCENT` each value is first divided by the column total of absolute finite values, $T_i = \\sum_j |v_{ij}|$, whenever that total is positive. The stack then always ends at 1, the axis is formatted as a percentage, and the totals disappear from the picture. Use it when you care about shares, and only then.",
      ],
      tex: [
        "\\tilde v_{ij} = \\frac{v_{ij}}{T_i}, \\qquad T_i = \\sum_j |v_{ij}|",
      ],
    },
    {
      heading: "Smooth lines",
      body: [
        "`Interpolation::Smooth` is not a spline through anything clever. It is a [monotone cubic](https://en.wikipedia.org/wiki/Monotone_cubic_interpolation) Bézier: for neighbouring points $P_k$ and $P_{k+1}$ with spacing $h_k = x_{k+1} - x_k$ and slope $d_k$, the tangent at an interior point is a weighted harmonic mean of the two neighbouring slopes, and it is set to zero when they disagree in sign.",
        "The two Bézier control points sit one third of the way along the segment, each offset vertically by tangent times $h/3$. The zero-tangent rule is what stops the curve from overshooting: a smooth line never goes higher than the highest of the two points it joins, which a plain cubic spline cheerfully does.",
      ],
      tex: [
        "m_k = \\begin{cases} 0 & d_{k-1} d_k \\le 0 \\\\[4pt] \\dfrac{3\\,(h_{k-1}+h_k)}{\\frac{2h_k + h_{k-1}}{d_{k-1}} + \\frac{h_k + 2h_{k-1}}{d_k}} & \\text{otherwise} \\end{cases}",
        "C_1 = \\bigl(x_k + \\tfrac{h_k}{3},\\; y_k + m_k \\tfrac{h_k}{3}\\bigr), \\quad C_2 = \\bigl(x_{k+1} - \\tfrac{h_k}{3},\\; y_{k+1} - m_{k+1}\\tfrac{h_k}{3}\\bigr)",
      ],
    },
  ],
  use: [
    "x is ordered and numeric (distance, speed, depth, time-like) and you want to see how one or more values change along it.",
    "Several series share one x: you want them in one tooltip, aligned.",
    "Parts of a total over x: stacked areas, or percent stacking when shares are the point.",
    "A value has a limit: draw it as a threshold line or a shaded band, or let the line change colour when it crosses.",
    "Two measures with different units, such as torque and power over rpm, each on its own axis.",
  ],
  avoid: [
    "x is a clock. [Time Series](/charts/time-series) gives you time ticks, timezones and compare.",
    "x is a category. Joining *Monday* to *Tuesday* with a line implies Monday slowly turns into Tuesday.",
    "x is not ordered, or you have several y values at one x. [XY Chart](/charts/xy-chart) keeps rows as given.",
    "You have six series that all wander around the same range. The result is a plate of spaghetti. Pick the two that matter and mute the rest.",
  ],
  read: [
    "Read the axis first, especially the left one. A line that goes up sharply on a 94 to 96 axis is a calm line on a 0 to 100 axis.",
    "Find the direction and the breaks. Gaps are real: the default behaviour is that a missing value breaks the line, and that is the chart telling you data was missing.",
    "For stacks, read the top edge as the total and the thickness of a band as that series' own value. Only the bottom band sits on the baseline, so compare the others by thickness, not by height.",
    "Hover to see every series at the same x. Click a legend entry to show only that series. Ctrl-click or Cmd-click toggles one.",
  ],
  story: {
    title: "Two axes, one gearbox",
    paragraphs: [
      "Tomasz built electric motor controllers for a company that made cargo bikes in a converted tram depot in Gdansk. His job that month was to explain to the sales team why the new motor felt weaker than the old one, even though the datasheet said it was stronger.",
      "The sales team did not want to hear about motor constants. They wanted a picture. Specifically they wanted a picture that could be sent to a customer who had written, in capital letters, that the bike felt like pushing a fridge up a hill.",
      "The measurement was simple: torque in newton-metres and power in watts, both against rotation speed. Two units, one x axis, and a very obvious temptation to draw two charts and let the reader align them with their finger.",
      "He used `<Trend>` from bui-charts instead, with torque as one `<TrendSeries>` and power as a second one on its own right-hand axis, coloured to match the line. Old motor was dashed, new motor solid, thresholds drawn as a dashed line at the controller's current limit.",
      "The chart made the answer obvious in about four seconds. The new motor had far more peak power, but it came at 480 rpm higher than the old one. At the low rpm of climbing a hill the torque curve was actually 12 per cent below the old motor. The datasheet quoted the peak, the customer lived at the bottom.",
      "Tomasz sent the chart to firmware with one line, asking for a field-weakening profile that moved the torque up in that region. Three weeks later a new curve appeared on the same chart, and the low rpm torque was 9 per cent above the old motor. The customer was sent the new chart and a bike, and replied with a single exclamation mark.",
      "The fridge, reportedly, has since been returned to the kitchen.",
    ],
  },
  pitfalls: [
    "**Joining points that should not be joined.** Use `Points` for raw samples, `StepAfter` for a value that holds until it changes, and `Line` for continuous measurements.",
    "**Stacking different units.** Stacks add numbers. Adding Nm to hp gives you a number in no unit at all. Stack like with like.",
    "**Truncated axes.** Auto ranges are chosen to fit the data, not to be honest. Fix the min to zero for bars and areas.",
    "**Unsorted x.** Trend expects x to go up. If it does not, lines join in the wrong order and you should use the XY chart.",
    "**Smoothing as decoration.** A smooth line has no data between the points. It is a presentation choice, not a measurement.",
  ],
  related: [
    { slug: "time-series", why: "The same chart with a time axis, timezones, annotations and time compare." },
    { slug: "xy-chart", why: "When x is not ordered or you have several y values at one x." },
    { slug: "candlestick", why: "When each x carries open, high, low and close rather than one value." },
    { slug: "horizon", why: "When you have dozens of series and need each in a thin strip." },
  ],
  media: [
    { kind: "youtube", id: "81Bv0unqrDY", title: "Plotting Non Time-Series Data on the X-axis in Grafana | Prescient", channel: "Prescient", note: "Shows plotting non-time data on the x-axis in Grafana, the idea behind the trend panel." },
  ],
}
