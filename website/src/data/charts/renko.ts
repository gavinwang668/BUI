import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A chart that only draws a brick when the price has moved a fixed amount, and ignores the clock.",
  intro: [
    "Time is a strange thing to put on the x axis of a price chart. The market does not care that it is 10:35. Nothing happens for twenty minutes, and then everything happens in ninety seconds, and your chart gives both stretches the same width, as if they were equally important. [Renko](https://en.wikipedia.org/wiki/Renko_chart) is the chart that stops pretending.",
    "The name comes from *renga*, the Japanese word for brick, and the chart is exactly that: a staircase of equal-sized bricks. You choose a brick size, say 2 dollars. Every time the price moves 2 dollars beyond the last brick, a new one is drawn, diagonally up or down. If the price twitches by 1.50, nothing is drawn. If it moves 7 dollars in a minute, three bricks appear at once.",
    "The result is a chart that has no gaps, no wicks, and no opinion about the time of day. Trends turn into long runs of one colour. Chop turns into a short zigzag you can ignore. It is a rare chart where being less informative is the feature.",
  ],
  math: [
    {
      heading: "Bricks from closes",
      body: [
        "Renko takes only the closes $p_1, p_2, \\dots$ and a brick size $s$ (`box_size`). The grid is anchored by rounding the first finite close down to a multiple of $s$, which gives the base $b$, and the first brick range is $[b, b + s]$. Write the current brick's low and high as $\\ell$ and $h$.",
        "A new up brick is drawn whenever the price reaches one whole size above the top of the current brick, and a new down brick whenever it falls one whole size below its bottom. If a single close crosses several sizes, the loop draws one brick per size, so a big move yields a stack.",
      ],
      tex: [
        "b = s \\left\\lfloor p_1 / s \\right\\rfloor, \\qquad [\\ell, h] = [b,\\, b + s]",
        "p \\ge h + s \\;\\Rightarrow\\; \\text{up brick } [h,\\, h+s], \\quad \\ell \\leftarrow h,\\; h \\leftarrow h + s",
        "p \\le \\ell - s \\;\\Rightarrow\\; \\text{down brick } [\\ell,\\, \\ell - s], \\quad h \\leftarrow \\ell,\\; \\ell \\leftarrow \\ell - s",
      ],
    },
    {
      heading: "The two-brick reversal",
      body: [
        "After an up brick, the new low $\\ell$ is the old high, and the brick's top is $h$. A down brick needs the price to fall to $\\ell - s$, which is two sizes below the top of the last brick. This is the standard Renko rule: reversing costs twice what continuing does, and that is what keeps the chart from flipping on every wobble.",
      ],
    },
    {
      heading: "Box size and axes",
      body: [
        "If you do not give a `box_size`, the chart uses one twenty-fifth of the price range in the data, rounded to two significant figures. Each brick is one row: it is drawn with the same candle painter as a candlestick, with the brick's start as the open and its end as the close, coloured up or down by direction. The x axis is the brick number (an integer count, not time), so a hover tells you brick $k$ and its two prices, not a date.",
      ],
      tex: [
        "s_{\\text{default}} = \\operatorname{round}_{2\\,\\text{sig}}\\!\\left(\\frac{\\max p - \\min p}{25}\\right)",
      ],
    },
  ],
  use: [
    "You care about price trends and want to delete time from the picture, so quiet and busy stretches are weighted by how much the price moved.",
    "You want a visually simple trend filter: a run of same-coloured bricks, and a reversal that is large enough to be worth noticing.",
    "You have closes at any resolution: hourly bars, daily bars, individual trades. The chart takes only `close`.",
    "You want to mark [support and resistance](https://en.wikipedia.org/wiki/Support_and_resistance): levels where bricks repeatedly stop and reverse show up as flat tops and bottoms.",
  ],
  avoid: [
    "You need to know when something happened. The x axis counts bricks, so the same run of bricks may be a minute or a month.",
    "You need volume or intraday high and low. The chart discards everything except the closes.",
    "Your brick size is guessed. Too small and the chart is as noisy as the price. Too large and it hides every trade you cared about.",
    "You want a chart that adapts to volatility. The brick size is fixed, so a calm market and a wild one will look the same size until the price actually moves.",
  ],
  read: [
    "Follow the staircase. A run of up bricks is an uptrend. The length of the run is how far the price has gone, in brick sizes.",
    "Look for reversals. A turn needs two bricks' worth of movement against the trend, so a colour change means a real move, not a flinch.",
    "Mark flat spots. Several bricks that stop at the same level suggest the price is meeting resistance or support there.",
    "Hover a brick to see its start and end prices. The x axis is brick number, so the tooltip will not show a time.",
  ],
  story: {
    title: "Twelve bricks and a long lunch",
    paragraphs: [
      "Naledi ran treasury for a company that imported coffee, and so, without ever wanting to, she had a view on the Brazilian real. The firm bought beans in dollars and sold in rand, and once a quarter somebody had to decide whether to hedge now or wait. The someone was Naledi.",
      "She had tried a daily line chart, and it was useless for this. The currency drifted sideways for three weeks, spiked, and then drifted sideways again, and the spike was exactly what she was supposed to notice. The chart gave the three quiet weeks twenty times the screen space of the afternoon when it mattered.",
      "What she wanted to know was simple. Is the currency in a real move, or is it only wobbling? She did not care about Tuesday. She cared about whether the price had gone a meaningful distance in one direction.",
      "She fed a year of daily closes into `<Renko>` in a small desktop tool built on bui-charts, with a box size of 0.05. The three quiet weeks collapsed into two bricks. The week of the spike expanded into a run of twelve of the same colour, and a flat ceiling of four bricks above it.",
      "It was the first time she had seen the quiet weeks and the spike as proportions of what they were worth. She hedged half the quarter's exposure when the thirteenth brick failed to appear, at a rate about 4% better than the following month's average.",
      "The finance director, who had asked for a dashboard of the whole market and was prepared to be disappointed, asked instead why the chart seemed to have skipped Thursday. Naledi explained that nothing happened on Thursday, and the chart had agreed.",
      "That is the whole product pitch, and she has used it ever since: a chart that only speaks when there is something to say.",
    ],
  },
  pitfalls: [
    "**Brick size is the whole game.** The chart is only as meaningful as the size you pick. Try several, and choose the one where noise disappears but real moves stay.",
    "**There is no time axis.** Two bricks next to each other can be seconds apart or a month apart. Do not compare brick counts across different date ranges.",
    "**Lag at reversals.** A reversal needs two brick sizes against the old trend, so the chart always confirms a turn after it has happened.",
    "**Big gaps make stacks.** A single close that jumps several sizes draws all the bricks at once, so a gap shows as a tall column of bricks rather than a gap.",
    "**Bricks are not trades.** Each brick edge is on a multiple of the size, so the prices shown are rounded to the grid, not the real fills.",
  ],
  related: [
    { slug: "point-figure", why: "When you also want to discard time but prefer a column of marks with a configurable reversal." },
    { slug: "kagi", why: "When you want reversals of a set size drawn as a single connected line." },
    { slug: "heikin-ashi", why: "When you want to keep time on the axis and smooth the candles instead." },
  ],
  media: [
    {kind:"youtube",id:"o3U8Yb2PLcc",title:"Introduction to Renko Charts: Tutorial",channel:"TradingView",note:"Official tutorial on how Renko bricks are formed and read."},
  ],
}
