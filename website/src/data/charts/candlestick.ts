import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Open, high, low and close for each period, drawn as one candle, with volume underneath.",
  intro: [
    "A line chart of a price throws away almost everything that happened in a day. It keeps one number, usually the close, and pretends the rest of the day was quiet. A candlestick keeps four: where the period opened, where it closed, and how far it strayed up and down on the way. That is a lot of story for a few pixels of width.",
    "The form is usually traced to Japanese rice traders in the eighteenth century, with the merchant [Munehisa Homma](https://en.wikipedia.org/wiki/Munehisa_Homma) often named as the person who made it famous. The details of that tale are better told by people who have read the sources than by a documentation page, but the design has clearly survived for a reason. Western traders found it in the 1990s and have been drawing it on every screen since, usually in front of a green and red glow.",
    "Do not let the finance flavour fool you. Anything that has a first, a last, a smallest and a largest value per bucket is a candlestick in waiting: the latency of each minute, the temperature of each day, the queue depth of each hour. A candle says where the bucket started, where it ended, and how wild it got in between. The only thing it will not do is predict next Tuesday, which is why the people who read tea leaves for a living still prefer it.",
  ],
  math: [
    {
      heading: "One bucket, four numbers",
      body: [
        "For each period $t$ you supply an open $o_t$, a high $h_t$, a low $l_t$ and a close $c_t$ (and optionally a volume $v_t$). The candle is a **body** between $o_t$ and $c_t$ and a thin **wick** from $l_t$ to $h_t$. Both are placed on a shared linear y scale, the same one used by [Trend](/charts/trend), so the axis is rounded outwards to nice numbers in the usual way.",
        "Parts you do not supply are not an error. If there is no open, the candle opens at the previous close, which is what happens in a market that never closes. If you give only a close, the whole chart still draws: each candle spans previous close to close.",
      ],
      tex: ["o_t \\leftarrow c_{t-1} \\quad \\text{when no open is given}"],
    },
    {
      heading: "Colour: which way did it go",
      body: [
        "Colour is a function of direction. With the default `ColorStrategy::OpenClose` a candle is *up* when its close is at or above its open, and *down* otherwise. With `ColorStrategy::CloseClose` the comparison is against the previous close, and the candle is up when $c_t \\ge c_{t-1}$. The two can disagree: a stock that opens far below yesterday's close and ends the day slightly higher is an up candle by one rule and a down candle by the other.",
        "Equal values count as up, with one exception: the very first candle under `CloseClose` has no previous close and is drawn flat. Under `CloseClose`, rising candles are drawn hollow, outlined in their colour, which is the convention many trading tools use.",
      ],
      tex: [
        "\\text{up}_t^{\\text{open-close}} = [\\,c_t \\ge o_t\\,], \\qquad \\text{up}_t^{\\text{close-close}} = [\\,c_t \\ge c_{t-1}\\,]",
      ],
    },
    {
      heading: "Volume and bar width",
      body: [
        "In the default `CandlesVolume` mode the plot is shared: candles use the price scale and volume bars use their own scale, drawn at half opacity so they stay behind the candles. The `Candles` and `Volume` modes give the whole height to one of them.",
        "[Moving averages](https://en.wikipedia.org/wiki/Moving_average) and bands are ordinary `<TrendSeries>` children with the full Trend styling, drawn over the candles.",
      ],
    },
  ],
  use: [
    "Each x holds a **range**, not a single value: a trading day, a minute of latency, a day's temperature.",
    "You care about the spread within a period as well as where it ended.",
    "You want to overlay a moving average, a band or a target on the same axes.",
    "Volume or activity should be read alongside price, in one panel.",
  ],
  avoid: [
    "You have one value per point. Candlestick with only a close draws bodies that are just the change since the last candle, which is a fine way to disguise a line chart as a lot of rectangles. Use [Time Series](/charts/time-series).",
    "Your audience does not read candles. Explaining which end of the body is the open will take longer than reading the data.",
    "Periods are very uneven in size, such as one tick and then a whole week, so the candles are not comparable.",
    "You want to show that patterns predict the future. They do not, whatever the green glow suggests.",
  ],
  read: [
    "Read the body first: the filled part runs from open to close, and its colour says whether it went up or down under the chosen strategy.",
    "Then the wicks. A long upper wick means the price went high and was pushed back. A long lower one means the opposite. Both are the range the line chart hid.",
    "Look at volume under the candle. A big move on small volume and a big move on large volume are different events.",
    "Hover a candle for open, high, low, close and volume.",
  ],
  story: {
    title: "The five-minute outage that wasn't in the average",
    paragraphs: [
      "Ines was an SRE at a company that sold appointment reminders by text message. On the dashboard the checkout latency was a calm blue line sitting at about 180 milliseconds, and it had been that calm blue line for months, including during the incident the customer success team kept bringing up in meetings.",
      "The incident was real. Customers had written in. Some had been charged twice. And the latency chart, which was an average per minute, showed a polite little bump to 260 ms and then went back to being calm, like a man who has been told to relax.",
      "An average is a very efficient way to hide a minute in which 3 requests took 40 seconds and 9,000 took 180 milliseconds. Ines knew that. She needed a chart that would show the worst request in every minute along with the first and last, so that anyone looking could see the length of the tail without opening a query console.",
      "She rebuilt the panel with `<Candlestick>` from bui-charts: for each minute, the first latency as open, the last as close, the minimum as low and the maximum as high, with request count as volume. A moving average went on top as a `<TrendSeries>`, and she picked blue and orange for up and down so nobody would need a colour chart.",
      "During the incident window the candles stopped looking like candles and started looking like flagpoles. Wicks ran up to 41 seconds, at the same minutes the double-charge tickets arrived, and the volume bars dipped just before, which is what a retry storm looks like from the outside.",
      "Armed with that picture she got the retry policy changed in a single afternoon. The next month there were two wicks above 5 seconds instead of forty-three, and the customer success team found something else to bring up in meetings.",
      "The calm blue line is still on the dashboard, in the corner, labelled *average*. It has been asked to reflect on that.",
    ],
  },
  pitfalls: [
    "**Colour rules differ between tools.** Open-close and close-close colouring give different candles for the same data. Pick one and say which.",
    "**Gaps in time.** Weekends and holidays leave holes unless your x values are sequence numbers. Decide whether a gap is information.",
    "**Red and green only.** Pick `up` and `down` colours that read for colour-blind viewers, such as blue and orange.",
    "**Treating a single close as OHLC.** With close only, each candle opens at the previous close, which draws bodies but adds no information.",
  ],
  related: [
    { slug: "heikin-ashi", why: "Candles smoothed with an averaging rule, for when the raw ones are jumpy." },
    { slug: "renko", why: "When time is noise and you only want price moves above a fixed size." },
    { slug: "time-series", why: "When you have one value per timestamp and no range." },
    { slug: "trend", why: "For moving averages and bands, which are drawn with the same series options." },
  ],
  media: [
    { kind: "youtube", id: "AOz1YPOKvEs", title: "How To Read Candlestick Charts FAST (Beginner's Guide)", channel: "Wysetrade", note: "Beginner guide to reading open, high, low and close from candles." },
  ],
}
