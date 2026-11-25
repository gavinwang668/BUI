import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Candlesticks that each remember the one before, so a trend looks like a run of one colour.",
  intro: [
    "A normal [candlestick chart](https://en.wikipedia.org/wiki/Candlestick_chart) is honest to a fault. It tells you exactly where each period opened, closed, peaked and bottomed out, and the result, on a restless market, is a row of candles alternating red and green like a badly planned flower bed. You came to ask which way the price is going. You were told what it did, in great detail, every five minutes.",
    "Heikin-Ashi is a Japanese term usually translated as \"average bar\", and that is the whole technique. Instead of drawing each period's raw open, high, low and close, you draw smoothed ones, where each candle is built partly from the one before. The noise cancels, the colour stays the same for as long as the trend does, and the pullbacks that would have been a red candle become a slightly shorter green one.",
    "The price you pay is that the candles are no longer real. The close of a Heikin-Ashi candle is not where anything traded. That is the trade-off, and it settles how to use the chart: to read direction, and never to place an order.",
  ],
  math: [
    {
      heading: "The recurrence",
      body: [
        "Let $O_t, H_t, L_t, C_t$ be the real open, high, low and close of period $t$, and write the smoothed values with a bar. The new close is the average of the four real prices. The new open is the midpoint of the previous smoothed candle's body, which is where the memory comes from. The first candle has no predecessor, so its open is the midpoint of the real open and close.",
      ],
      tex: [
        "\\bar C_t = \\frac{O_t + H_t + L_t + C_t}{4}",
        "\\bar O_t = \\frac{\\bar O_{t-1} + \\bar C_{t-1}}{2}, \\qquad \\bar O_1 = \\frac{O_1 + C_1}{2}",
        "\\bar H_t = \\max(H_t,\\, \\bar O_t,\\, \\bar C_t), \\qquad \\bar L_t = \\min(L_t,\\, \\bar O_t,\\, \\bar C_t)",
      ],
    },
    {
      heading: "Why it smooths",
      body: [
        "Because $\\bar O_t$ is an average of earlier smoothed values, it is an [exponentially weighted](https://en.wikipedia.org/wiki/Exponential_smoothing) memory of the past: each earlier candle contributes half as much as the one after it. That memory is the reason the body of one candle sits near the middle of the last one, so a sudden small reversal rarely flips the colour.",
        "The cost is lag. A smoothed candle trails the real price, and by an amount that depends on how fast the market moved. It also means the highs and lows are clipped to include the smoothed body, so the wicks are never inside it.",
      ],
    },
    {
      heading: "What the chart draws",
      body: [
        "The transformed rows go to the same candle painter as the plain candlestick chart, so `mode` (candles, OHLC bars, volume) and `candle_style` work in the same way, and the volume series, if given, is not smoothed. A candle is up when its smoothed close is above its smoothed open. The up and down colours default to the theme's good and bad colours. The x axis is a time axis by default and tooltips show all values at the hovered period. The values shown in the tooltip are the smoothed ones.",
      ],
    },
  ],
  use: [
    "You want to see the direction of a trend without the day-to-day noise: long runs of the same colour mean a persistent move.",
    "You want a visual cue for indecision: small bodies with long wicks on both sides are where the trend tends to stall.",
    "You already have OHLC data at a fixed period and want a smoother view of the same bars, with no change to the time axis.",
    "You are holding a position and want to know whether the move is still intact before looking at exact prices elsewhere.",
  ],
  avoid: [
    "You need real prices: entries, stops, fills or profit calculations. The smoothed open and close are averages, not trades. Use [Candlestick](/charts/candlestick).",
    "The market gaps a lot. The smoothed open ignores the gap, so a large overnight move can look like it did not happen.",
    "You want to react quickly. The smoothing delays every signal, and in a sharp reversal you will be a bar or two behind.",
    "You want to filter by price size rather than time. Time still dictates the bars here, for size-based moves use [Renko](/charts/renko).",
  ],
  read: [
    "Look at the colour run. Many same-coloured candles in a row is a trend. The length of the run is the persistence.",
    "Look at the wicks. A strong up candle has almost no lower wick, a strong down candle has almost no upper one. Wicks on both sides with a small body means the market could not decide.",
    "Compare body sizes. Shrinking bodies along a run usually mean the trend is losing force before the colour changes.",
    "Hover a candle to see its smoothed open, high, low and close (and volume if you gave it). Remember these are averages, not trades.",
  ],
  story: {
    title: "The candle that stopped flinching",
    paragraphs: [
      "Gustavo traded the front-month crude future for a small proprietary firm, and was good at it in the unglamorous way: small positions, stops respected, a spreadsheet for everything. His one weakness, which he shared with most people who stare at charts for a living, was flinching.",
      "On a five-minute candlestick chart, crude oil produced a red candle about as often as a green one, even in a sharp rally. Gustavo would see a red candle in the middle of an up-move, feel his stomach lurch, and exit. Then he would watch the price continue upward for another forty minutes. The firm's risk report had a column for this. It was called \"premature exits\" and it was kind.",
      "What he wanted was not another indicator on top of the chart. He wanted the chart he already trusted, with the noise reduced, so that a pullback inside a trend would look like a pullback and not like an alarm.",
      "He rebuilt the screen in a small desktop app with bui-charts, putting `<HeikinAshi>` next to a plain `<Candlestick>` on the same feed. Same data, same period, one smoothed and one raw. He kept the raw one for entries, because that is where the real prices are, and used the smoothed one only to decide whether to stay in.",
      "On the first rally he used it, the smoothed chart showed eleven green candles in a row, with tiny lower wicks, while the raw one showed seven greens and four reds. He stayed in. The move ran for 34 more minutes and added about 1.8 dollars a barrel.",
      "Over the quarter his premature exits fell from 23 to 9, and the time he held winning trades rose by about a third. The red candles still appeared on the raw chart, as before. He simply stopped taking them personally.",
      "The smoothed chart has one flaw, which is that it makes everything look obvious in hindsight. Gustavo has asked that nobody tell him this.",
    ],
  },
  pitfalls: [
    "**The prices are not real.** The close of a smoothed candle is an average of four prices and may sit where nothing traded. Never read an entry or a stop off this axis.",
    "**It lags by construction.** Because each open depends on the previous candle, the colour change arrives after the real turn. Fast V-shaped reversals look especially late.",
    "**The first candle is special.** With no previous candle, the open is the midpoint of the real open and close. It takes a few candles for the chart to settle, so read the first handful with care.",
    "**Gaps disappear.** The smoothed open is built from the previous smoothed body, so a gap between sessions is not visible as one.",
    "**Smoothing is not forecasting.** A long green run says what happened, and gives no promise that it continues.",
  ],
  related: [
    { slug: "candlestick", why: "When you need the real open, high, low and close of each period." },
    { slug: "renko", why: "When you want to filter by price move rather than by smoothing in time." },
    { slug: "kagi", why: "When you want to see only reversals of a set size, with the thickness showing supply and demand." },
  ],
  media: [
    { kind: "youtube", id: "f-4ch_x5jlE", title: "Heikin Ashi Candles", channel: "The Moving Average", note: "Explains how Heikin Ashi candles are calculated and read." },
  ],
}
