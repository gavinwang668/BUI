import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Each value plotted against the one that follows it, so the sequence draws its own rule.",
  intro: [
    "Here is a cheap trick that is better than it sounds. Take a list of numbers in order. Plot the first against the second, the second against the third, the third against the fourth, and so on. A sequence that has no structure fills the plot like spilled flour. A sequence that follows any rule at all, even a messy one, lays itself along a curve or clusters into a shape. The chart is a camera that photographs the rule instead of the numbers.",
    "The name comes from [Henri Poincaré](https://en.wikipedia.org/wiki/Henri_Poincar%C3%A9), who studied the way a system's trajectory repeatedly pierces a chosen surface, and looked at the points where it does. Plotting each return against the next is the lag-1 version of that idea. Cardiologists adopted it for [heart-rate variability](https://en.wikipedia.org/wiki/Heart_rate_variability): plot each beat-to-beat interval against the one after it, and a healthy heart makes a comet, an arrhythmic one makes a fan, and a metronome makes a dot.",
    "It is a remarkable amount of diagnosis for a scatter plot, and it needs no model, no units and no assumptions beyond *the order matters*.",
  ],
  math: [
    {
      heading: "Lagged pairs",
      body: [
        "Take the values you extract with `value` in row order: $x_0, x_1, \\dots, x_{N-1}$. For a `lag` $k$ (1 by default, and never less than 1) the chart plots the $N - k$ points below. The first coordinate is on the horizontal axis, the second on the vertical.",
      ],
      tex: [
        "\\bigl(x_n,\\; x_{n+k}\\bigr), \\qquad n = 0, 1, \\dots, N - k - 1",
      ],
    },
    {
      heading: "The identity line",
      body: [
        "The dashed reference line is $y = x$, drawn from the smallest to the largest finite value in the data (non-finite values are ignored when finding the range, though they are not removed from the pairs). A point on the line has $x_{n+k} = x_n$: the sequence did not change. Above the line the next value is larger, below it smaller. Turn the line off with `identity={false}`.",
        "The axis labels use the optional `label`, so with label `RR` they read RRₙ and RRₙ₊₁. The sign of $x_{n+k} - x_n$ is the one thing you can read directly from where a point sits relative to the diagonal.",
      ],
    },
    {
      heading: "What shapes mean",
      body: [
        "If the sequence is a deterministic map, $x_{n+1} = g(x_n)$, then every point lies on the graph of $g$ and the cloud collapses to a curve. A fixed point satisfies $g(x^{*}) = x^{*}$ and so sits where the curve meets the diagonal. A period-two orbit appears as two dots mirrored across the diagonal. Noise added to a map thickens the curve. Noise added to a measurement of it thickens it in a different, tilted way.",
        "For heart-rate data two spreads are traditionally reported: the standard deviation of the points across the diagonal and along it. The chart does not compute them, and they are cheap to get from the same two columns, but the shape of the cloud is where you should look first.",
      ],
      tex: [
        "\\mathrm{SD1}^{2} = \\tfrac12\\,\\mathrm{Var}(x_{n+1} - x_n), \\qquad \\mathrm{SD2}^{2} = \\tfrac12\\,\\mathrm{Var}(x_{n+1} + x_n)",
      ],
    },
  ],
  use: [
    "You have a time-ordered series and want to know in one picture whether it is steady, periodic, noisy or chaotic.",
    "You are looking at heart-rate RR intervals, ECG-derived data or anything else where beat-to-beat variability matters.",
    "You suspect a one-dimensional rule, such as a population model or a feedback loop, and want to see if the data lay on a curve.",
    "You want to compare two recordings by the shape of their clouds, not by their means.",
  ],
  avoid: [
    "Order is meaningless. If you shuffle your data and it looks the same, the chart is just a scatter of a distribution with extra steps. Use a [Histogram](/charts/histogram).",
    "You need to see change over time. The plot discards the time axis. Slow drift looks like a smear along the diagonal.",
    "Sampling is irregular. A lagged pair assumes the gap between observations is the same each time.",
    "The system has several interacting variables. One lag of one variable shows only a shadow of it.",
  ],
  read: [
    "Check where the cloud sits relative to the dashed diagonal. Centred on it means no trend. A systematic offset means the series is rising or falling.",
    "Look at its width and orientation. A tight cloud along the diagonal means each value predicts the next. A round cloud means almost no memory.",
    "Look for structure: a curve, two clusters, a loop, a fan. Each one is a claim about the rule behind the data.",
    "Hover a point to read its pair of values. Isolated points far from the cloud are single events, such as an ectopic beat, worth finding in the raw data.",
  ],
  story: {
    title: "The marathon runner and the Tuesday ectopics",
    paragraphs: [
      "Wen was a sports physiologist who worked with a university rowing squad, which is a group of people who regard sleep as a weakness and a resting heart rate of 38 as a personality. One of them, a second-year called Ali, had started feeling flat in training and could not say why. His average resting heart rate was perfectly normal. It always is, until it is not.",
      "The squad wore chest straps that recorded every beat interval overnight. Wen had two weeks of them for Ali, about 40,000 numbers a night, and no sensible way of reading a list of 40,000 numbers. A chart of the intervals in time looked like a seismograph that had been told to stay calm.",
      "What she wanted was a picture where regular things would look regular and irregular things would stand out by *where* they landed, not how tall they were. Heart-rate researchers have a long-standing answer to that: plot each interval against the next.",
      "She gave `<PoincareMap>` the interval column with `label=\"RR\"` and a small point size, because there were a lot of points. A healthy night looks like a comet pointing along the diagonal. Ali's first five nights did. The sixth had two thin streaks, one above the cloud and one below it, mirrored through the diagonal.",
      "Mirrored streaks are the signature of a premature beat: a short interval followed by a compensating long one. The sixth night had 212 such pairs, and the seventh had 340. The first thing Wen noticed was that the cloud itself had also tightened into a smaller, rounder blob, which in this context is the shape of a body under strain.",
      "She sent Ali to a cardiologist, who found nothing alarming but a lot of stress hormones and a training load that had grown by 40 percent in a fortnight. Two lighter weeks later the streaks were gone and the comet was back. Ali complained that the chart had taken away his best excuse for a bad erg test.",
      "Wen kept the picture on her wall. It is the only chart in her office that can tell a tired rower from a rested one without being told which is which.",
    ],
  },
  pitfalls: [
    "**Too many points.** Long recordings make a solid blob. Shrink `point_size` or look at a section of the record at a time.",
    "**Lag choice.** Lag 1 shows the short-term rule. A periodic signal may only reveal itself at a lag near its period, so try several values of `lag`.",
    "**Non-finite values.** NaNs are kept in the pairs, so a gap in the data becomes missing points, but the identity line is only based on the finite ones. Clean the data first if gaps are common.",
    "**Equal axes.** Both axes share a variable but not a scale setting, so a visually perfect diagonal need not be at 45 degrees. Judge position against the dashed line, not against your sense of angle.",
  ],
  related: [
    { slug: "phase-portrait", why: "When you have the equations of motion and want the whole flow rather than a sample of one orbit." },
    { slug: "bifurcation", why: "When the rule has a parameter and you want to see how its long-run values change as the parameter moves." },
    { slug: "xy-chart", why: "When the two quantities are different variables and not a series and itself." },
  ],
  media: [
    {kind:"youtube",id:"-TgvrZ_sQLs",title:"Poincaré Maps - Dynamical Systems | Lecture 28",channel:"Jason Bramburger",note:"University lecture defining Poincaré maps and how they reduce a flow to a discrete map."},
  ],
}
