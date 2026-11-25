import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "One big number, how it got there, and whether it has moved since you last looked.",
  intro: [
    "Most dashboards are built by people who love charts, and it shows. You open one to answer a single question, *is the thing on fire?*, and you are handed a wall of lines, axes, legends and a time picker. The stat panel is the opposite instinct. It takes one value, makes it as large as the tile allows, and gets out of the way.",
    "The format is old and humble: the scoreboard. A speedometer reading, the number on a petrol pump, the total at the bottom of a receipt. Dashboards inherited it as the \"single stat\", and [Grafana's Stat panel](https://grafana.com/docs/grafana/latest/panels-visualizations/visualizations/stat/) is the version most people have seen: a headline number, a faint [sparkline](https://en.wikipedia.org/wiki/Sparkline) behind it for context, and a colour that changes when the number crosses a line you care about. This chart is a port of that design, so if you know it, you already know this.",
    "The sparkline is what keeps it honest. A bare number says 94%. A number with a sparkline says 94%, and it was 40% an hour ago. Same digits, very different Tuesday.",
  ],
  math: [
    {
      heading: "From rows to one number",
      body: [
        "Take a column of readings $y_1, \\dots, y_n$ in time order. A stat tile first reduces them to a single value $v$ with the calculation you picked in `reduce`. The default is the last finite reading. Others are the mean, minimum, maximum or sum.",
        "Non-finite readings (missing samples, NaN) are skipped before reducing. If you ask for `Reduce::All` instead, nothing is reduced: every row becomes its own tile and there is no sparkline.",
      ],
      tex: [
        "v = \\operatorname{reduce}(y_1,\\dots,y_n), \\qquad \\text{e.g. } v = y_n \\;\\text{ or }\\; v=\\tfrac{1}{n}\\textstyle\\sum_i y_i",
      ],
    },
    {
      heading: "Percent change and the sparkline",
      body: [
        "With `show_percent_change`, the tile shows the change from the first finite reading $y_a$ to the last finite reading $y_b$, the same way Grafana does. Note that this is the first-to-last change, not the change of the reduced value. If $y_a = 0$ the quotient is not finite and nothing is shown.",
        "The sparkline maps time to the tile's width and value to its height. Writing $[x_{\\min}, x_{\\max}]$ for the time range and $[y_{\\min}, y_{\\max}]$ for the value range of that tile's own data, a point lands at the fractions below. If the series is flat, the range is widened by 10% of its magnitude (or set to 0 to 1 when the value is zero) so the line sits in the middle instead of dividing by zero.",
      ],
      tex: [
        "\\Delta = \\frac{y_b - y_a}{y_a}\\times 100",
        "u = \\frac{x - x_{\\min}}{x_{\\max}-x_{\\min}}, \\qquad w = \\frac{y - y_{\\min}}{y_{\\max}-y_{\\min}}",
      ],
    },
    {
      heading: "Colour and layout",
      body: [
        "The colour comes from a value mapping if one matches, otherwise from the thresholds: the step whose lower bound is the largest one not above $v$. Percentage thresholds are resolved against `min` and `max`, which default to the smallest and largest number in the data.",
        "Each tile then picks one of four layouts from its own shape. With width $W$ and height $H$, a tile counts as wide when $W/H > 2.5$. A wide tile with a sparkline and $H > 50$ px puts the name left, the value right and the sparkline in the bottom half. A tall tile with a sparkline needs $H > 100$ px to stack name, value and sparkline. Text is sized to fit by measuring it, and with several tiles every tile takes the size that fits the longest name and value, so the row lines up.",
      ],
    },
  ],
  use: [
    "You have one question per tile and the answer is a number: current CPU, orders today, open incidents, p99 latency.",
    "The reader will glance at it from across a room or on a wall monitor. A background colour (`BackgroundSolid`) is readable from further away than any axis.",
    "You want a row of KPIs with a bit of history each. Pass a `Frame` and you get one tile per field, with matching text sizes.",
    "The status is a code, not a quantity: map `200` to OK and `503` to Down with value mappings and let the tile show the word.",
  ],
  avoid: [
    "The shape of the history is the story. A sparkline has no axis and no labels on purpose, if people need to read values off the line, use [Time Series](/charts/time-series).",
    "You want the number against a known range with limits drawn on it. That is what [Gauge](/charts/gauge) is for.",
    "You are comparing many values with each other. Tiles of different colours are hard to rank. A [Bar Gauge](/charts/bar-gauge) lines them up on one scale.",
    "The number is only meaningful with its distribution around it, such as a latency that is fine on average and terrible at the tail.",
  ],
  read: [
    "Read the number first. The tile is sized so that it is the biggest thing on the screen.",
    "Check the colour. It is the threshold step the value reached, so green or red tells you whether to keep reading.",
    "Glance at the sparkline for direction only. Each sparkline is scaled to its own range, so two tiles with similar-looking lines can differ by orders of magnitude.",
    "If the percent change is on, it compares the first and last readings in the tile, and the arrow shows the direction. There is nothing to click through to: the tile is the whole answer, which is the point.",
  ],
  story: {
    title: "The dashboard nobody had to open",
    paragraphs: [
      "Marguerite ran support operations for a company that sold smart thermostats, which is to say she answered for every house that was too cold at 6 a.m. and every customer who had opinions about it. Her team's wall monitor showed a Grafana-style board of eleven panels, and by her own count three of them had never been looked at on purpose.",
      "The trouble was the morning stand-up. It started at 9:00 and spent the first eight minutes on one question: how bad is the queue? Somebody would lean toward the screen, squint at a multi-line chart of ticket volume and say something like \"looks a bit higher than yesterday?\", which is what people say about every chart.",
      "Marguerite wanted four numbers a person could read from the coffee machine: open tickets, median first response in minutes, tickets older than a day, and today's volume against the same hour last week. Each needed a colour that meant something, and each needed just enough history to separate \"high\" from \"high and climbing\".",
      "She built the board in a small desktop app using bui-charts. A single `<Stat>` fed with a frame of the four fields gave her four equal tiles, text sized to the longest label, sparklines underneath. She set thresholds per field, switched the colour mode to `BackgroundSolid`, and flipped `percent_change_color` to `Inverted`, because for ticket counts a rise is bad news and a tile that turned green when things got worse would have been funny exactly once.",
      "The first morning, the aged-tickets tile was a deep, unmissable red with a 62% arrow beside it. Nobody leaned toward the screen. Somebody said \"it's the weekend backlog\" from the doorway, and that was the whole discussion.",
      "Over the next month the stand-up lost its first eight minutes. Median first response dropped from 41 to 27 minutes, mostly because a red tile is difficult to defer and a line chart is easy to. Aged tickets stayed under 30 for three straight weeks, a number that had not previously been seen in the wild.",
      "The other seven panels are still on the old board, as a monument. The wall monitor now shows four squares, and the squares are the only thing anyone has ever complained about not being large enough.",
    ],
  },
  pitfalls: [
    "**Reduce chooses the story.** `Last` is \"now\", `Mean` is \"over this period\", `Max` is \"the worst moment\". The same data gives three different numbers, so say in the title which one you picked.",
    "**Sparklines are scaled per tile.** A flat-looking line on a tile whose values range between 99.1 and 99.3 is as dramatic as a spike elsewhere. Do not compare slopes across tiles.",
    "**Percent change is first to last.** If the series dipped and recovered, the change is small even though the middle was ugly. And a first value of zero produces no change at all.",
    "**Colour direction.** Up is not always good. For errors, latency and cost use `PercentChangeColor::Inverted`, and set thresholds so that the high end is the bad end.",
    "**Fixed text sizes.** Setting `value_size` stops the text from fitting the tile. That is useful when panels must match, and clipped when the number grows an extra digit.",
  ],
  related: [
    { slug: "gauge", why: "When the number has a known range and the limits are what matter." },
    { slug: "bar-gauge", why: "When you need to compare several values against the same range." },
    { slug: "time-series", why: "When the history itself is the answer and people need to read it." },
  ],
  media: [
    { kind: "youtube", id: "mB9FU0myZo8", title: "Introducing Percent Change in Grafana Stat Visualizations (10.3) | Grafana", channel: "Grafana", note: "Grafana's own demo of the stat panel's percent change display, a close relative of this chart." },
    { kind: "youtube", id: "LrkR6_hcqPc", title: "Stat Visualization Percent Change Color Mode in Grafana 11.1 (Demo) | Grafana", channel: "Grafana", note: "Grafana demo of the stat panel percent change colour mode." },
  ],
}
