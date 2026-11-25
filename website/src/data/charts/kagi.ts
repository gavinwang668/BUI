import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "One connected line that changes direction only when the price reverses by a set amount, thick when demand is winning.",
  intro: [
    "Most price charts are a diary: here is what happened at 9:30, here is what happened at 9:35. A [Kagi chart](https://en.wikipedia.org/wiki/Kagi_chart) is closer to a sketch of the argument. It does not care when things happened. It only cares which way the price was going, and whether the last move against it was big enough to count as a change of mind.",
    "It comes from Japanese markets and is usually said to be named after the word for a key, which the zigzags resemble. Western traders met it mostly through Steve Nison's books on Japanese charting. The drawing rule is almost comically simple: draw a vertical line while the price continues in one direction, and when it reverses by at least your chosen amount, step sideways and draw a new line the other way.",
    "The part that earns the chart its keep is the thickness. When the price breaks above an earlier high, the line turns thick (yang). When it breaks below an earlier low, the line turns thin (yin). So a glance at the width tells you whether buyers or sellers have been in charge, which is more than a line chart will ever say, for the price of a slightly odd-looking picture.",
  ],
  math: [
    {
      heading: "Building the lines",
      body: [
        "Kagi works on closes $p_1, p_2, \\dots$ and a reversal amount $r$. It keeps a list of line end-points, starting with the first close. Until the price has moved by at least $r$ from that start, nothing is drawn, and the first move that does fixes the first direction $d \\in \\{+1, -1\\}$.",
        "Afterwards, for each new close $p$, let $e$ be the end of the current line. If $p$ is further in the current direction, the line is extended: $e \\leftarrow p$. Otherwise, if $p$ is at least $r$ against the direction, the direction flips and a new line is started that ends at $p$. Moves against the direction smaller than $r$ are ignored.",
      ],
      tex: [
        "(p - e)\\,d > 0 \\;\\Rightarrow\\; e \\leftarrow p, \\qquad |e - p| \\ge r \\;\\Rightarrow\\; d \\leftarrow -d,\\; \\text{new line ending at } p",
      ],
    },
    {
      heading: "Yang and yin",
      body: [
        "Each line is labelled thick (yang) or thin (yin) by remembering the last turning points. An up line becomes yang when it ends above the previous up-line's top (the shoulder). A down line becomes yin when it ends below the previous down-line's bottom (the waist). Otherwise the line keeps the thickness it had. The first line is yang if it goes up, yin if it goes down.",
        "Both series are drawn as step lines: the line moves horizontally between lines and vertically along each one. Thick lines are drawn 3.5 px wide and thin ones 1.5 px. A thick segment is also joined to its neighbour so there are no gaps where the thickness changes.",
      ],
    },
    {
      heading: "The default reversal",
      body: [
        "If `reversal` is not given, the chart uses 2% of the mean close, rounded to two significant figures, so the threshold scales with the price level. The x axis is the line number, an integer count, and the legend lists Yin and Yang.",
      ],
      tex: [
        "r_{\\text{default}} = \\operatorname{round}_{2\\,\\text{sig}}\\bigl(0.02\\,\\bar p\\bigr)",
      ],
    },
  ],
  use: [
    "You want the swings of a price that matter, and a single line to follow, with no gaps and no time axis.",
    "You are looking for trend changes, where the line switches from thin to thick or the reverse, as a clearer signal than a colour flip.",
    "You have closes at any resolution and a reversal size that is meaningful to you: 2%, or a fixed dollar amount.",
    "You want a simple, historic way to separate demand-led moves from supply-led ones.",
  ],
  avoid: [
    "You need to know when things happened. The x axis counts lines, not minutes.",
    "You need the intraday range. Only closes are used. Highs and lows are ignored.",
    "Your reversal amount is arbitrary. A small one makes a hectic zigzag and a large one makes a few lines. Try a couple of values before trusting any one.",
    "Your audience has never seen one. The thick and thin lines need a sentence of explanation. [Candlestick](/charts/candlestick) needs none.",
  ],
  read: [
    "Follow the line from left to right. Each vertical stretch is a move in one direction, and each sideways step is a reversal.",
    "Read the thickness. Thick means the price has just broken above an earlier high, thin means it has broken below an earlier low.",
    "Watch for the switch. Thin to thick is the price taking out a previous high, and it is the signal this chart is best known for.",
    "Hover to read the levels. The x axis is the number of the line, so it will give you a price and a count, not a date.",
  ],
  story: {
    title: "The line that changed its mind",
    paragraphs: [
      "Aurelio kept the books for a family-owned olive oil exporter in Andalusia, and for reasons of inheritance, also kept the portfolio. His uncle had left a handful of shares in a shipping company, which the family held in the quiet hope that they would one day be explained to them.",
      "Aurelio did not want to become a trader. He wanted to know, twice a year, whether to sell. A line chart had told him nothing: the price wandered up and down within a few percent for months, and each time he looked at it, it looked like a different story. A candlestick chart looked like a row of tiny arguments.",
      "He wanted a chart that ignored the small stuff and only spoke up when the price truly changed direction, and one that could say whether the buyers or the sellers had the upper hand while it did so.",
      "A friend who worked at a brokerage showed him Kagi. He fed two years of weekly closes into `<Kagi>` in a small desktop tool built with bui-charts, set `reversal` to 3 euros, and was surprised how little there was to look at. The two years became 19 lines.",
      "More surprising was the thickness. For eleven lines the chart was thin. Then it went thick for five, and then, in the seventeenth line, thin again, with the price falling below the earlier low. That thin line was the first signal in two years that anything at all had changed.",
      "He sold two thirds of the holding at 41 euros. The shares closed the year at 33, and the family's one-third remainder is described by his aunt as \"the part we are still being reasonable about\".",
      "Aurelio now says the chart is mostly useful because of everything it declines to show him. He considers this the same quality he looks for in a good accountant.",
    ],
  },
  pitfalls: [
    "**Reversal size changes the picture completely.** Too small and the chart is as jittery as the price. Too large and a real turn takes two reversals to show.",
    "**No time axis.** Two adjacent lines may be a day or a year apart. Do not read line counts as durations.",
    "**Thick does not mean up.** Thickness shows a break of an earlier high or low. A thick line can be heading down, if it is the stretch before the next shoulder is broken.",
    "**The first line is a guess.** Until the price has moved by one full reversal from the start, nothing is drawn, and the first direction comes only from that first move.",
    "**Only closes.** Intraday spikes and wicks never appear, which is the point, until the one time it is not.",
  ],
  related: [
    { slug: "renko", why: "When you want to drop time too, but draw fixed-size bricks instead of one line." },
    { slug: "point-figure", why: "When you want to count how far each move went in boxes, as columns of marks." },
    { slug: "candlestick", why: "When the real open, high, low and close of each period are what you need." },
  ],
  media: [
    {kind:"youtube",id:"Ityt8RdXNGI",title:"Kagi Charts on TradingView: Tutorial",channel:"TradingView",note:"Official tutorial on setting up and reading Kagi charts."},
    {kind:"youtube",id:"aN25hVo9ASc",title:"Kagi Charts Explained: The Complete Technical Analysis Guide",channel:"WaveLabs",note:"Explains the reversal rule and thick/thin line logic behind Kagi charts."},
  ],
}
