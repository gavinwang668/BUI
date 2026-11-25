import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "One estimate and its confidence interval per row, against a line of no effect.",
  intro: [
    "A single study is an anecdote with a p-value. Ten studies of the same question are a conversation, and the interesting part is whether the ten are saying roughly the same thing. A [forest plot](https://en.wikipedia.org/wiki/Forest_plot) lines them up so you can hear that: each study gets a row, a square for its best guess, and a horizontal whisker for how far wrong that guess could be.",
    "The chart grew up in medicine, where it is the standard picture of a [meta-analysis](https://en.wikipedia.org/wiki/Meta-analysis). If you have seen the logo of the [Cochrane](https://en.wikipedia.org/wiki/Cochrane_(organisation)) collaboration, you have looked at one: it is a forest plot of seven trials on steroids given to women at risk of premature birth, and the diamond that used to summarise them is a small monument to a treatment that was underused for years because nobody had put the trials in a single row. The name is usually explained as a joke about the trees: so many lines, and somewhere in there, the wood.",
    "It earns the page anywhere you have estimates with uncertainty and a meaningful reference value. Drug trials, A/B tests run in twelve countries, benchmark scores across seeds. The vertical line in the middle is the reference, and the question the chart asks every row is: do you cross it, or not?",
  ],
  math: [
    {
      heading: "What each row is",
      body: [
        "For study $i$ of $n$ you provide a point estimate $\\hat\\theta_i$, a lower bound $l_i$ and an upper bound $u_i$ (in the same units; the chart does not compute the interval, so it can be a 95% interval, a standard-error band or anything else you choose), and optionally a weight $w_i$. A reference value $\\theta_0$ draws the line of no effect, `null`, which defaults to $0$.",
        "Rows are drawn in the order you pass them, first row at the top. A row's vertical position is $y_i = n - 1 - i$, counting from zero, so the list reads the same way a table does.",
      ],
      tex: ["l_i \\;\\le\\; \\hat\\theta_i \\;\\le\\; u_i, \\qquad \\text{interval } i = \\bigl[\\,l_i,\\,u_i\\,\\bigr] \\text{ at height } y_i"],
    },
    {
      heading: "The weight and the square",
      body: [
        "When you pass a weight, the marker for each estimate is a square whose size is scaled between 6 and 20 pixels according to $w_i$. Without a weight every square is 9 pixels. In a meta-analysis the natural weight is the inverse variance of each study, so precise studies draw big squares and noisy ones draw small ones.",
        "The weight is not used for any computation: the chart does not pool the studies and draws no summary diamond. It draws what you give it.",
      ],
      tex: ["w_i = \\frac{1}{\\sigma_i^{2}} \\quad \\text{(a common choice)}"],
    },
    {
      heading: "Ratios and the log axis",
      body: [
        "Odds ratios, risk ratios and hazard ratios multiply: a ratio of $2$ and a ratio of $0.5$ are the same size of effect in opposite directions. On a linear axis they are not the same distance from the reference of $1$, which makes harmful effects look more dramatic than protective ones. Set `null={1.}` and a log `value_axis`, and equal ratios are equal distances.",
        "On the log scale the interval for a ratio is symmetric around its estimate, which is why papers report ratios with confidence intervals that look lopsided in the table and balanced in the plot.",
      ],
      tex: ["\\log \\mathrm{OR}_i \\pm 1.96\\,\\mathrm{SE}_i \\;\\Longrightarrow\\; \\mathrm{OR}_i \\cdot e^{\\pm 1.96\\,\\mathrm{SE}_i}"],
    },
  ],
  use: [
    "You have several estimates of the same quantity with their uncertainty: trials, sites, segments, model runs.",
    "The reference value matters: zero difference, a ratio of one, a target. The dashed line is the whole reason to draw this chart.",
    "You want readers to see consistency or its absence at a glance, not to read a table of 30 confidence intervals.",
    "You are comparing subgroups in one study. Forest plots are the usual picture for a subgroup analysis, where the sad thing to look for is a subgroup nobody planned to look at.",
  ],
  avoid: [
    "You have raw data, not estimates. Show the distribution with a [beeswarm](/charts/beeswarm) or a [histogram](/charts/histogram) and let the reader judge the spread.",
    "Your intervals do not mean the same thing across rows. A 95% interval beside a standard error beside a range is not a forest, it is a trap.",
    "You want to show a pooled result. This chart draws the rows only. The pooled estimate has to come from your analysis, and you can add it as a final row.",
    "You have hundreds of rows. Past 40 or so the labels have to go, and without labels it is just a lot of horizontal lines.",
  ],
  read: [
    "Find the dashed line first. It is the reference: no effect, no difference, a ratio of 1.",
    "For each row, ask whether the whisker crosses that line. If it does, the study alone cannot rule out no effect.",
    "Then compare rows. Whiskers that overlap each other tell you the studies agree. One that sits far from the rest is the one to ask questions about.",
    "Look at the squares' sizes. A big square with a tight whisker carries the result. A small square with a very long whisker adds a voice and little else.",
    "Hover a row for its name, the estimate and, if you passed one, its weight.",
  ],
  story: {
    title: "Seven hospitals, one dashed line",
    paragraphs: [
      "Priya was a clinical informatics lead at a regional health network that ran a sepsis alert across seven hospitals. Each hospital had measured the same outcome after the alert went live: the odds of dying within thirty days. Each hospital had a report, a slide deck and a very strong opinion.",
      "The reports were honest, and no two looked alike. One gave an odds ratio of 0.62 with a confidence interval she had to find in a footnote. One said \"significant improvement\". One said \"no significant change\" with the same interval width and a point estimate that was higher. The chief medical officer asked her for a one-page answer, which in that building meant three paragraphs and an apology.",
      "What she needed was not a verdict but a comparison on a shared axis: seven estimates, seven intervals, one reference. And she needed to keep it honest. Odds ratios live on a log scale, so a plain axis would make the two hospitals that did worse look much worse than the ones that did better looked good.",
      "She put the rows into a `<Forest>` from bui-charts with the odds ratio as the estimate, the 95% bounds as low and high, the number of patients as the weight, `null` set to 1 and a log value axis. Six lines of Rust, and one comment from the colleague who reviewed it: \"that's it?\"",
      "It was it. Five hospitals sat to the left of the dashed line, two of them with tiny whiskers because they were the big ones. Two sat across it, with intervals so long that their squares were barely visible. The picture said what none of the reports could: the alert helps, the effect is consistent, and two sites simply do not have enough patients to say much.",
      "The chief medical officer approved the rollout to the remaining three hospitals the same afternoon. In the next quarter the network's estimate across all ten sites came in at an odds ratio of 0.71, close to what the first seven suggested. Priya got the credit for the number. The plot got the credit for the speed.",
      "Her only complaint was that the two small hospitals now called to ask why their squares were so small. She told them it was because the chart was being polite.",
    ],
  },
  pitfalls: [
    "**Linear axis for ratios.** If the estimates are odds, risk or hazard ratios, pass `null={1.}` and a log `value_axis`. Otherwise halving and doubling are different distances from the line.",
    "**Crossing the line is not proof of anything.** A whisker that touches the reference means that study did not detect an effect. It does not mean there is none.",
    "**Weights that do not match the intervals.** Square sizes should reflect precision. If you size by something unrelated (sample size when the variance is dominated by another factor), the plot suggests more confidence than you have.",
    "**Sorting to taste.** Rows are drawn in input order. Sorting by estimate makes the picture cleaner and the data look more consistent than the study order did. Say so if you do it.",
  ],
  related: [
    { slug: "dumbbell", why: "When each row has two values to compare, not an estimate and an interval." },
    { slug: "lollipop", why: "When you have one value per item with no uncertainty to show." },
    { slug: "bland-altman", why: "When the question is agreement between two methods, with limits around the difference." },
  ],
  media: [
    { kind: "youtube", id: "VK4KBPlG54Q", title: "How to Interpret a Forest Plot in a Meta-Analysis", channel: "This Is Why with Dr. Busti", note: "Walks through the line of no effect, ratios, weights and heterogeneity in the order you meet them on a real plot." },
    { kind: "youtube", id: "26i1RCM03D8", title: "How to Interpret a Meta-Analysis Forest Plot?", channel: "Betanho Martins Medical", note: "A beginner-friendly tour of the squares, whiskers and diamond, good if you have never read one." },
  ],
}
