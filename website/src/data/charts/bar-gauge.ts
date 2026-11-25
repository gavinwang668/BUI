import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A filled bar for each value against a known range, coloured by how close it is to trouble.",
  intro: [
    "A bar chart answers *how big is each thing?* A bar gauge answers a different and more anxious question: *how full is each thing?* Disk at 83%. Error budget at 12%. A queue holding 9,400 of a possible 10,000. The bar has a known start and a known end, and the interesting information is where the fill sits between them and what colour it has turned.",
    "The idea comes from the instrument panel: the fuel gauge, the thermometer tube, the signal meter on an old stereo. All of them exist because a number is slow to read and a length is quick. The dashboards of the last decade took that one trick and put forty of them on a screen, which is why you will find it described as a *bar gauge* in monitoring tools and *[bullet graph](https://en.wikipedia.org/wiki/Bullet_graph)* by people who have read Stephen Few.",
    "In bui-charts there are three looks. The default gradient shows which thresholds a bar has crossed. The LCD style chops the bar into discrete cells, like a meter in a recording studio, which is easier to count at a distance. The basic style is a quiet rectangle for dashboards that already have enough going on. All three share one idea: the range is fixed, and the bar is honest about it.",
  ],
  math: [
    {
      heading: "Fill",
      body: [
        "Each bar has a value $v$ and the gauge has a range $[a, b]$ set by `min` and `max`. The filled fraction is the position of the value inside the range. It is capped at one on the high side, and a value that is not a number is drawn empty.",
        "The bar's length in pixels is the available extent $E$ along the gauge's axis times that fraction. The fraction is the same whether the bars are drawn as rows or columns. Only the axis changes.",
      ],
      tex: [
        "f = \\min\\!\\left(\\frac{v - a}{b - a},\\ 1\\right), \\qquad \\text{length} = f \\cdot E",
      ],
    },
    {
      heading: "Thresholds",
      body: [
        "Colours come from threshold steps, each a value and a colour, with the first step covering everything below the second. `Thresholds::absolute` states steps in the units of the data. `Thresholds::percentage` states them as percentages of the range, so a step $p$ is at the value below, and the thresholds follow `min` and `max` when those change.",
        "The default gauge turns from the theme's *good* colour to its *bad* colour at 80. The step's position along the bar is its fraction $s$, which is where the gradient marks it.",
      ],
      tex: [
        "x_p = a + (b - a)\\,\\frac{p}{100}, \\qquad s = \\frac{x - a}{b - a} \\ \\text{(absolute)}, \\quad s = \\frac{p}{100} \\ \\text{(percentage)}",
      ],
    },
    {
      heading: "LCD cells",
      body: [
        "The LCD style divides the extent into $n = \\lfloor E / 12 \\rfloor$ cells of about 12 px including the gap. Cell $k$ stands for the value $a + (b - a)\\,k/n$ and takes its colour from that value, so the bar changes colour exactly where the thresholds are. It is lit when that value is at or below the current one, and unlit cells are drawn faintly.",
        "The consequence is that LCD bars are quantised. A value that is 41% of the way across lights 41% of the cells rounded down to a whole cell, and that is the price for being countable.",
      ],
      tex: [
        "\\text{cell } k \\text{ lit} \\iff a + (b - a)\\frac{k}{n} \\le v, \\qquad n = \\left\\lfloor \\frac{E}{12} \\right\\rfloor",
      ],
    },
  ],
  use: [
    "Each value has a **known range** and you care how full it is: disk, memory, quota, SLO budget, queue against capacity.",
    "You are comparing many things against the same limits, such as forty hosts with the same disk size.",
    "The reader needs a verdict (fine, watch it, fix it) faster than they need a number. Colour thresholds do the work.",
    "You have a table of series and want the latest value, or the mean or the peak, of each.",
  ],
  avoid: [
    "The values have no natural maximum. A bar with an invented max is a bar chart with a confusing coat on.",
    "The point is how a value changed. Use [Trend](/charts/trend) or [Time Series](/charts/time-series). One gauge is one moment.",
    "You need to compare sizes rather than fullness. A plain bar chart is the honest one there.",
    "A single value with a target and context deserves a [Bullet](/charts/bullet) chart, which has room for the comparative marker.",
  ],
  read: [
    "Look at the colour before the length. Green or red tells you whether to keep reading.",
    "Then compare lengths down the list. Bars share one scale, so a longer bar is a fuller one, across rows or columns.",
    "In LCD mode, count lit cells: it is easier at a distance than judging a smooth edge.",
    "Hover a bar for its exact value. The name sits with each bar, and the text can be set to show names, values or both.",
  ],
  story: {
    title: "Forty-one hosts and a quiet red bar",
    paragraphs: [
      "Gustavo ran the storage fleet for a university research cluster in Valparaiso: forty-one machines, each with a 12-terabyte scratch disk, each used by a rotating cast of graduate students who regarded deletion as a thing that happened to other people's files.",
      "The monitoring was a table. Forty-one rows, hostnames in one column, used percentage in the next. Every Monday Gustavo scrolled it, squinted at the numbers, and tried to remember which of them were considered bad. A rule in the alerting system paged him at 95%, which is roughly like a fire alarm that goes off when the building is already hot.",
      "In February a disk filled to 100% in the middle of a three-day simulation. The run was lost. The student who owned it was gracious, in the way people are when they plan to mention it in their thesis acknowledgements.",
      "Gustavo wanted the table to look like what it was, a list of things with a limit. He used `<BarGauge>` from bui-charts: one bar per host, fed from the same time series he already collected, reduced to the latest value of each field, with percentage thresholds at 70 for amber and 85 for red so the colours moved if a disk was ever resized.",
      "He picked the LCD display because the wall monitor was across the room, and the lit cells were countable from the door. The default horizontal rows with names on the left fit all forty-one on one screen.",
      "Within a week the wall told a story the table had hidden. Eleven bars were amber, and three of those had been climbing by roughly 4 per cent a day. Gustavo emailed those three owners with a screenshot and a date, and not a word more.",
      "No simulation has been lost since. The monitor in the corridor is now referred to as the thermometer, and students walk by and, with a kind of fond dread, count their cells.",
    ],
  },
  pitfalls: [
    "**Invented maximum.** If `max` is a number you made up, every bar's fullness is a statement about your imagination. Use a real capacity.",
    "**Thresholds that do not follow the range.** Absolute thresholds stay put when you change `min` and `max`. Percentage ones move with them. Pick the one you mean.",
    "**Pale bars with white text.** Use `value_mode={ValueMode::Text}` so the numbers stay readable on a light colour.",
    "**Palette colours used as a verdict.** `GaugeColor::Palette` tells bars apart, it does not judge them. Reserve red for bad.",
    "**Building threshold lists in render.** A `vec![..]` in your component is rebuilt every time. Put them in a `static`.",
  ],
  related: [
    { slug: "gauge", why: "One value on a dial, when a single number deserves the whole panel." },
    { slug: "bullet", why: "One bar with a target marker and qualitative ranges behind it." },
    { slug: "stat", why: "When you want the number big and a sparkline behind it." },
  ],
  media: [
    { kind: "youtube", id: "7PhDysObEXA", title: "Beginners Guide - Visualizing Bar Gauges in Grafana | Grafana Labs", channel: "Grafana", note: "Official Grafana walkthrough of the bar gauge panel and its options." },
  ],
}
