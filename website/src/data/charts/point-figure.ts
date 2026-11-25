import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Columns of X for rising prices and O for falling ones, each mark a fixed-size box of movement.",
  intro: [
    "[Point and Figure](https://en.wikipedia.org/wiki/Point_and_figure_chart) charting is old enough that it predates the pocket calculator and, in a sense, the chart. It was a way of keeping track of prices in the nineteenth century using nothing but pencil and squared paper: when the price rose a box, you wrote an X in the next square up, when it fell, you wrote an O. You did not need a clock. You did not need a ruler. You needed graph paper and a certain amount of patience.",
    "That origin explains everything about how it looks. The chart is a grid of boxes, each worth a fixed price amount. A column of Xs is a rally. A column of Os is a decline. When the price turns by enough boxes, you move one column to the right and start the other symbol. Time is absent. Only price movement is recorded. Flat stretches contribute nothing, and big moves contribute many marks.",
    "What you get is a picture of supply and demand in which levels are the main character. Where Xs and Os keep turning around the same row, the price has found a floor or a ceiling, and you can see it without a ruler.",
  ],
  math: [
    {
      heading: "Prices to boxes",
      body: [
        "Each close $p$ is converted to a box index by flooring it onto a grid of size $s$ (`box_size`). Only the box matters from here on: two closes in the same box are the same event, and the exact price is thrown away.",
        "If you do not give a box size, the chart uses one twenty-fifth of the range of the closes, rounded to two significant figures. The vertical axis shows prices, with each box drawn at its centre.",
      ],
      tex: [
        "k = \\left\\lfloor p / s \\right\\rfloor",
        "s_{\\text{default}} = \\operatorname{round}_{2\\,\\text{sig}}\\!\\left(\\frac{\\max p - \\min p}{25}\\right)",
      ],
    },
    {
      heading: "Columns and the reversal rule",
      body: [
        "The chart builds columns left to right. Let the current column be rising, covering boxes up to $h$. A new close in box $k$ extends the column if $k > h$. If $k \\le h - m$, where $m$ is the reversal in boxes (`reversal`, default 3), the column ends and a new falling column starts. That new column begins one box below the old top, at $h - 1$, and runs down to $k$. The mirror rule applies to a falling column with bottom $\\ell$: it extends if $k < \\ell$ and flips if $k \\ge \\ell + m$, starting at $\\ell + 1$.",
        "The first column starts when a close lands in a different box from the first close, in the direction it moved. A close that changes nothing, or that is smaller than the reversal against the column, adds no mark at all.",
      ],
      tex: [
        "k > h \\Rightarrow h \\leftarrow k, \\qquad k \\le h - m \\Rightarrow \\text{new O column from } h-1 \\text{ down to } k",
      ],
    },
    {
      heading: "Drawing the marks",
      body: [
        "Each box in a column gets one mark at height $(k + \\tfrac12)\\,s$ and the column number as its x position. A rising box is an X: two crossing strokes. A falling box is an O: a small ellipse. The marks use the theme's good and bad colours, the x axis counts columns, and there are no tooltips, because the picture is the point.",
      ],
    },
  ],
  use: [
    "You care about support and resistance. Levels where columns keep turning are visible as rows where the marks repeat.",
    "You want to ignore both time and small moves. Only a change of `reversal` boxes starts a new column, so wobbles inside a trend vanish.",
    "You want a price chart with a built-in rounding: every move is a whole number of boxes, which makes targets and stops easy to count.",
    "You have closes, and nothing else. Only `close` is used, at any resolution.",
  ],
  avoid: [
    "You need exact prices or when something happened. Closes are rounded onto boxes and the x axis is a column number.",
    "You are looking at a thin series with large steps. If most moves are less than a box, the chart will be almost empty.",
    "You want volume or intraday range. Neither is used.",
    "You want a chart a casual audience will decode without help. X and O columns are a convention that has to be explained once. [Candlestick](/charts/candlestick) is more familiar.",
  ],
  read: [
    "Look at the columns. A tall column of Xs is a strong rally of that many boxes. A short one is a minor bounce.",
    "Compare neighbouring columns. Each new column of Xs that reaches higher than the last is an uptrend, if it fails, the price is stalling.",
    "Find the rows where several columns turn. These are levels where the price has repeatedly been rejected or supported.",
    "There is no hover. The y axis gives the price, and the box size is the unit for every distance you count.",
  ],
  story: {
    title: "The graph paper that beat the screen",
    paragraphs: [
      "Hilde taught an evening course on technical analysis at a community college, and had, over six years, gathered a number of strong opinions about her students. Chief among them was that they would buy any chart that moved. Real-time candles, flashing indicators, three monitors. She was fairly certain that none of it helped.",
      "She wanted to show a class of twenty how little of a price chart is information. The usual lecture, \"most of what you see is noise\", had been given on this course for years and had been accepted by everyone and acted on by nobody.",
      "She needed a demonstration so concrete that it could not be shrugged off: the same price history, but drawn in a way that threw away almost everything except the moves that counted. And she needed it to be something the students could count on their own fingers.",
      "She loaded a year of hourly closes for a mid-cap stock into `<PointFigure>` from bui-charts, set `box_size` to 1 and left the reversal at the default of three. Eight thousand hourly closes became 31 columns. She projected the candlestick version of the same data on one half of the screen and the Point and Figure chart on the other.",
      "The students could count the boxes. They could see that the stock had stalled five times at the same row, 74, and that the one time it broke through, the next column ran to 83. Nobody needed to be told what resistance was. It was a row on the chart, with marks on it.",
      "Course evaluations that term included the line \"finally understood support and resistance\", which Hilde has kept. Three students bought graph paper. One of them kept a chart by hand for a year and said, correctly, that it was the only chart that had never lied to him about being interesting.",
      "Hilde's view is that if a price move cannot be seen on graph paper, it is probably not a move.",
    ],
  },
  pitfalls: [
    "**Box size and reversal are both judgement calls.** A small box and a reversal of 1 reproduce the noise. A large box and a reversal of 5 miss everything. Try a few and look for the setting where the levels become clear.",
    "**The x axis is columns, not time.** A narrow chart can span years and a wide one a week. Never read the horizontal distance as duration.",
    "**Each new column starts one box inside the old one.** A column of Os starts below the top of the Xs, not at it, which is why the columns appear to interlock.",
    "**Closes are rounded onto boxes.** The exact price disappears. Two closes in the same box are identical to the chart.",
    "**Quiet markets draw nothing.** A price that never moves a whole box adds no marks, and the chart will look empty for exactly the period when nothing happened.",
  ],
  related: [
    { slug: "renko", why: "When you want bricks of fixed size in a single staircase rather than columns of marks." },
    { slug: "kagi", why: "When you want the swings drawn as one line with thickness showing the balance of supply and demand." },
    { slug: "candlestick", why: "When time and the exact open, high, low and close matter." },
  ],
  media: [
    {kind:"youtube",id:"snxW3d41xkg",title:"Understanding Point & Figure Chart and How to Trade with it? | Edelweiss Wealth Management",channel:"Nuvama Wealth",note:"Explains the X and O columns, box size and reversal rules."},
  ],
}
