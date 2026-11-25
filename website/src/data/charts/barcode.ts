import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Every value as a thin tick on a number line, one strip per group.",
  intro: [
    "The honest way to show a distribution is to show the data. A histogram shows a summary of the data, a box plot shows a summary of the summary, and a density curve shows what a smoothing function thinks about the data. A barcode skips all of that: it takes each value and puts a thin vertical mark where it belongs on the axis. If you have 300 response times, you get 300 marks.",
    "Statisticians call close cousins of this a [rug plot](https://en.wikipedia.org/wiki/Rug_plot) (the little ticks along the bottom of a density plot, like fringe on a carpet) or a strip plot. The idea is old and unglamorous, which is part of its charm. Nothing is binned. Nothing is smoothed. Where the marks bunch up the strip looks dark, where they thin out it looks pale, and the gaps are as informative as the clusters.",
    "That is the case for it: it is the chart you reach for when you do not yet trust a summary. Draw the barcode first, look at where the values actually sit, and then decide whether you need a histogram to say it more loudly.",
  ],
  math: [
    {
      heading: "Data in, ticks out",
      body: [
        "Take a list of values $x_1, \\dots, x_N$, each with an optional group label. Values that are not finite (missing, NaN, infinity) are dropped. The rest are sorted ascending, which does not change where a tick appears but makes the drawing order deterministic.",
        "Each remaining value becomes one tick at horizontal position $x_i$ on the value axis. There is no binning and no estimator. The position is the data.",
      ],
    },
    {
      heading: "Groups and strips",
      body: [
        "Groups are numbered in the order of their first appearance in your data: sort your rows and you sort the strips. With $G$ groups, the strip for group $g$ is centered on the integer $g$ of the category axis, and the axis runs from $-0.5$ to $G - 0.5$ so that every strip has the same amount of room.",
        "Each tick is a bar that spans a fixed band around its strip's center. A tick belonging to group $g$ runs from the floor to the top below, which means a strip is $0.7$ of a category slot tall and there is $0.3$ of empty space between neighbouring strips.",
      ],
      tex: ["\\text{floor}_g = g - 0.35, \\qquad \\text{top}_g = g + 0.35"],
    },
    {
      heading: "Width, opacity and density",
      body: [
        "Ticks are 1.5 pixels wide by default (`tick_width`) and drawn at 80% opacity. That second number is the density estimator: where $k$ ticks land on the same pixel the strip is darker, roughly in proportion to $1 - 0.2^{k}$, so it saturates quickly. A barcode is good at showing *that* a region is crowded, not by how much.",
        "Because the chart does not bin, there is no bin width to choose, only a tick width. A narrower tick separates values that sit close together and a wider one makes sparse data easier to see. Neither changes which value a tick stands for.",
      ],
      tex: ["\\alpha_{\\text{pixel}} = 1 - (1 - 0.8)^{k}"],
    },
  ],
  use: [
    "You have up to a few hundred values and every one should be visible: response times of a small batch, test scores for a class, sensor readings from a short run.",
    "You are comparing groups on the same scale and care about gaps, clusters and outliers more than the mean.",
    "You are doing exploratory analysis and want to see what the data looks like before picking a summary.",
    "You need something compact: a barcode strip is a few pixels tall, so ten groups fit where one histogram would.",
  ],
  avoid: [
    "You have tens of thousands of values. The strip turns into a solid bar and nothing can be told apart. Use a [histogram](/charts/histogram) or a [ridgeline](/charts/ridgeline).",
    "You need to see how many values share one spot exactly. Identical values stack on top of each other. A [beeswarm](/charts/beeswarm) spreads them out.",
    "You want to compare shapes of many distributions at once. A [ridgeline](/charts/ridgeline) shows shapes. A barcode shows positions.",
    "Your values are heavily rounded. Integer data will form a comb of ticks, and the comb looks like structure that is only your rounding.",
  ],
  read: [
    "Read position along the axis: each tick is one observation, so the horizontal position is its value.",
    "Look for dark and pale stretches in a strip. Dark means many values live there, pale means few.",
    "Look for gaps, and for isolated ticks far from the rest. Those are your outliers, and in a barcode they cannot hide inside a bin.",
    "Compare strips by their position, not their height. All strips are the same height. The tick pattern is the information.",
    "The tooltip is off by default, since a thousand ticks make a bad hover target. Pass your own `tooltip` if you want one.",
  ],
  story: {
    title: "The gap in the middle",
    paragraphs: [
      "Marcus managed a small support team at a company that sold smart thermostats, and his team had one metric: median time to first reply. For six months it had sat comfortably at 14 minutes, and for six months a customer success director had been saying that this was a success.",
      "Marcus did not entirely believe it. The complaint emails he saw seemed to come from two kinds of people: customers who were answered in a few minutes, and customers who wrote the word \"still\" in the subject line. A median could easily describe a team that was good at one thing and terrible at another.",
      "He needed to see the 412 tickets from the last fortnight, not a number made from them. A histogram would have been fine, but he suspected that whatever he was looking for was a few tickets wide, and a bin would hide it.",
      "He pulled the reply times into a `<Barcode>` from bui-charts, one strip per shift (early, day, night), the value being minutes to reply. It was a single component, three props, and it took about as long to write as the Slack message asking whether anyone had seen the logs.",
      "The early and day strips were dense stretches of dark from 2 to 25 minutes. The night strip was something else: a handful of ticks near 5 minutes, then nothing, then a thin scatter of marks between 90 and 240. No bell, no tail, just a gap where the data should have been. The median of that strip happened to land in the empty middle, and 14 minutes was an average of nothing in particular.",
      "It turned out that the overnight shift was one person covering two time zones, who handled whatever was marked urgent first and then slept. Marcus added a second person to the night rotation. Two weeks later the night strip had filled in, and the share of tickets answered inside an hour went from 71% to 93%.",
      "The director still says the median was fine. In a technical sense it was, which is the most dangerous kind of fine.",
    ],
  },
  pitfalls: [
    "**Overplotting looks like a verdict.** When 80 values fall on the same few pixels the strip goes solid. That means \"crowded\", not \"exactly this many\". If you need counts, bin.",
    "**Rounded data forms combs.** Whole numbers, or readings from a coarse sensor, land on a lattice of ticks. Do not read structure into the spacing of a lattice.",
    "**Different scales per group are not supported.** All strips share the value axis, which is the point. Groups on different units do not belong in one barcode.",
    "**Tick width changes the feel.** A wide tick makes sparse data look more crowded than it is. Pick a width once and keep it when you compare charts side by side.",
  ],
  related: [
    { slug: "beeswarm", why: "When values collide and you want each one to get its own spot, at a modest cost in space." },
    { slug: "histogram", why: "When there are too many values for ticks and counts per bucket are enough." },
    { slug: "ridgeline", why: "When you want to compare the shapes of many groups instead of their individual values." },
  ],
  media: [
    { kind: "youtube", id: "Io-Ew7bWbcU", title: "Python Seaborn - 11|What is a Rug Plot in Python Seaborn and How to Create This", channel: "Learnerea", note: "The rug plot is the barcode's closest relative. This shows why drawing one tick per value is a useful first look at data." },
  ],
}
