import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Every value as a dot, nudged sideways just enough that no two dots overlap.",
  intro: [
    "Put a hundred dots on a number line and some of them will land on top of each other. You can make them see-through, which turns clumps into smudges. You can add random jitter, which makes the picture different every time you open it and, in a bad mood, slightly dishonest. Or you can do what the beeswarm does: place each dot at its true value along one axis, and use the other axis only to keep the dots from touching.",
    "The result looks like the insects it is named after, hovering where the data is dense. It is a one-dimensional scatter plot with the free dimension put to honest use. The horizontal position is exact. The vertical wobble is just packing, so it carries no information and the spread of a swarm is itself a rough picture of the density.",
    "That is why people like it more than a [box plot](https://en.wikipedia.org/wiki/Box_plot), which is a drawing of five numbers, and more than a [violin](https://en.wikipedia.org/wiki/Violin_plot), which is a drawing of a smoother. A beeswarm shows each observation, and you can still see the shape.",
  ],
  math: [
    {
      heading: "What is fixed and what is free",
      body: [
        "Each finite value $x_i$ is placed at exactly that position on the value axis. The perpendicular axis is a category axis: group $g$ is centered at the integer $g$, and a dot gets a perpendicular offset $d_i$ from the center. Offsets are limited to $|d_i| \\le 0.45$, so swarms in neighbouring groups never touch.",
        "The chart needs a notion of \"close\" in value. It takes it from the data: the spread $s$ is one ninetieth of the range of all the values, so roughly ninety dots fit side by side before they have to stack.",
      ],
      tex: ["s = \\frac{x_{\\max} - x_{\\min}}{90}"],
    },
    {
      heading: "The packing rule",
      body: [
        "Within a group the values are processed in ascending order. For each value, the algorithm tries offsets in turn: $0$, then $+h$, $-h$, $+2h$, $-2h$, and so on, where $h = 0.045$ is the step. It takes the first offset at which the new dot is clear of every dot already placed. Two dots are clear when their distance, measured with the horizontal gap scaled by $s$ and the vertical gap scaled by $h$, is at least 1.",
        "Only dots whose values are within $s$ of the new one can collide, and because the values are sorted the search can stop as soon as it reaches one that is further than that. A dot in an isolated region therefore always lands on the center line.",
      ],
      tex: [
        "d_i = \\text{first } o \\in \\{0, +h, -h, +2h, -2h, \\dots\\} \\text{ such that } \\sqrt{\\bigl(\\tfrac{x_i - x_j}{s}\\bigr)^2 + \\bigl(\\tfrac{o - d_j}{h}\\bigr)^2} \\ge 1 \\;\\; \\forall j \\text{ placed}",
      ],
    },
    {
      heading: "When the swarm runs out of room",
      body: [
        "If a dot needs an offset larger than $0.45$, the algorithm gives up on it and leaves it on the center line. In a very dense swarm you will therefore see a spine of overlapping dots in the middle. That is the chart telling you there are too many points for the available room: shrink `point_size`, give the chart more height, or switch to a [histogram](/charts/histogram).",
        "Dots are drawn at 5 pixels unless you pass `point_size`. Groups are numbered in the order of their first appearance, and each group gets its own colour from the theme.",
      ],
    },
  ],
  use: [
    "You have a few dozen to a few hundred observations per group, and each one is worth seeing: lab replicates, race times, survey scores.",
    "You want to compare groups and show the sample size at the same time. A group of six dots looks different from a group of six hundred, which a box plot hides.",
    "Your audience does not like abstractions. Dots are the most literal chart there is.",
    "You suspect two clusters (bimodality) that a mean or a box would flatten.",
  ],
  avoid: [
    "You have thousands of points per group. The swarm will fill its width and fall back to the center line. Use a [ridgeline](/charts/ridgeline) or a [histogram](/charts/histogram).",
    "You need the vertical position to mean something. Here it never does, if the second axis is data, you want a scatter plot.",
    "Values are tied a lot (counts, ratings from 1 to 5). Ties become neat piles, which are fine, but a bar chart of the counts may say it better.",
    "You need to compare an exact quantile. Dots do not give you a median. Draw a [barcode](/charts/barcode) with a marker, or compute the number and print it.",
  ],
  read: [
    "Horizontal position is the value. Vertical position inside a swarm means nothing. Ignore it as a coordinate.",
    "The width of a swarm at a given value tells you how crowded that value is. Where the swarm is wide, many observations share it.",
    "Count the dots if you need a sample size. Unlike a box plot, nothing is hidden.",
    "Compare groups by where their swarms sit and how they are shaped, not by their height on the page.",
    "Hover a dot for its value. The legend (if you turn it on with `legend`) names the groups by colour.",
  ],
  story: {
    title: "Two hundred runners and a suspicious bulge",
    paragraphs: [
      "Noor organised a small city half-marathon, and the year after a tough course change she wanted to know whether the new hill had broken anyone. The race had 214 finishers, a results spreadsheet and a volunteer who insisted that the hill was fine and that people were just out of shape.",
      "The obvious summary was the mean finishing time by age group. Noor wrote it down and then did not like it. Age groups had different sizes, some had six runners, and a mean of six runners is a rumour.",
      "She wanted to see every runner, grouped by age band, on a single time axis. No bins, because the bins would have to be wide enough to hide the exact thing she was looking for, and no random jitter, because she was going to show it to a committee and the committee asks \"why did you move my dots\".",
      "She loaded the finishers into a `<Beeswarm>` from bui-charts, with finishing time as the value and the age band as the group. One component, a closure for each of the two accessors, and a point size of 4 to fit the 214 dots comfortably.",
      "The 30 to 39 and 40 to 49 swarms looked like what she expected: a hump around two hours, with a few fast outliers on the left. The 50 to 59 swarm did not. It had a second small cluster about eleven minutes after the main one, six dots, all clearly separate. A mean would have averaged right over it.",
      "Six runners from one running club had stopped at the new hill to wait for a friend who was walking it. It was a lovely thing to do and a terrible thing for a time-based awards list. Noor added a note to the standings, moved the water station to the base of the hill, and emailed the club with an apology and a photo of the dots.",
      "The committee asked for the chart again next year. They said it was because it was useful. Noor suspects it was because they could find their own dot.",
    ],
  },
  pitfalls: [
    "**Reading the swarm's vertical spread as a measurement.** It is a packing artefact. Two swarms of equal density but different sizes will look different in width.",
    "**Too many dots.** When the swarm exceeds its room, dots fall back to the center line and overlap. If you see a dark spine, lower `point_size` or switch chart.",
    "**Different dot sizes across charts.** The spread is derived from the value range, so the same data on a different axis range packs differently. Compare dot patterns within one chart, not between two.",
    "**Group order.** Groups follow the order of their first row. Sort your data before you pass it in, or the strips come out in an accident.",
  ],
  related: [
    { slug: "barcode", why: "When you have more values than room for dots and want only positions, not packing." },
    { slug: "ridgeline", why: "When you would rather see smooth shapes for many groups than individual points." },
    { slug: "histogram", why: "When the sample is large and counts per bucket are the question." },
  ],
  media: [
    { kind: "youtube", id: "FiVevA8fBgg", title: "Beeswarm Plots (Including SHAP Values)", channel: "ML & DL Explained", note: "Covers how a beeswarm differs from a histogram and why the sideways spread carries no value." },
    { kind: "youtube", id: "OoYM6rVgGHI", title: "Violin and Beeswarm Plots", channel: "De Novo Software - FCS Express", note: "A side-by-side look at the smooth summary (violin) and the raw dots (beeswarm), which is the trade this chart is about." },
  ],
}
