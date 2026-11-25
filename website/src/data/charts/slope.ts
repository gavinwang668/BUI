import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "One line per item between two moments, so you can see who rose, who fell and who crossed whom.",
  intro: [
    "Take a table with two columns, last year and this year, and a few dozen rows. Tables are excellent at telling you what each number is and terrible at telling you what happened. You end up running a finger down both columns, doing subtraction in your head, and losing track around row eleven.",
    "A slope chart fixes this with the smallest possible amount of chart. There are two vertical positions, one for each moment. Each item is a line from its first value to its second. Up-sloping lines rose, down-sloping lines fell, steep ones changed a lot, and lines that cross swapped places. [Edward Tufte](https://en.wikipedia.org/wiki/Edward_Tufte) gave the form its popular name, slopegraph, in *The Visual Display of Quantitative Information* in 1983, where he used it to show government receipts as a share of GDP across countries.",
    "It is a very economical chart. There is no x axis to speak of, no gridlines worth the name, and nothing to decode. The only difficulty is that it rewards a small number of items, and punishes a large number with a bowl of spaghetti.",
  ],
  math: [
    {
      heading: "Two columns, a line each",
      body: [
        "Each row has a name $c_i$, a value $a_i$ at the first moment and a value $b_i$ at the second. The chart places the first moment at $x = 0$ and the second at $x = 1$, and draws one line per row between $(0, a_i)$ and $(1, b_i)$ against a shared y axis.",
        "The x axis is a category axis with your two labels (defaulting to Before and After), stretched slightly beyond both ends so that markers and value labels are not clipped. The padding runs from $-0.25$ to $1.25$.",
      ],
      tex: ["\\ell_i(x) = a_i + (b_i - a_i)\\,x, \\qquad x \\in \\{0, 1\\}"],
    },
    {
      heading: "Colour by direction",
      body: [
        "With `color_by_change` on (the default), each line is coloured by the sign of its change $\\Delta_i = b_i - a_i$: the theme's good colour when $\\Delta_i \\ge 0$ and its bad colour when negative. Zero counts as good. With it off, items cycle through the series palette instead, which is what you want when up is not good news, such as costs or error rates.",
        "Dots are always shown at both ends, and the value labels are on by default. The legend sits to the right and lists the items by name.",
      ],
      tex: ["\\text{colour}_i = \\begin{cases} \\text{good} & \\Delta_i \\ge 0 \\\\ \\text{bad} & \\Delta_i < 0 \\end{cases}"],
    },
  ],
  use: [
    "You compare the same items at two points in time, or under two conditions, and care about direction and rank change.",
    "You have between about 5 and 20 items. Enough to see a pattern, few enough to read each line.",
    "You want the audience to notice crossings: the item that overtook another, the one that fell below the pack.",
  ],
  avoid: [
    "There are more than two moments. Use a [bump](/charts/bump) chart for ranks over time, or a [time series](/charts/time-series) for values.",
    "Many items start or end at nearly the same value. The labels and dots pile up and the lines become a braid.",
    "The question is how far each item moved in absolute terms and the levels do not matter. A [dumbbell](/charts/dumbbell) is easier to measure.",
    "Items have very different scales, say 5 and 50,000. The small ones flatten against the bottom.",
  ],
  read: [
    "Take the two labels as your two moments, and ignore everything in between: there is no data there, only a line.",
    "Green lines went up, red lines went down, unless you turned that off. Steepness is the size of the change.",
    "Look for crossings. Where two lines cross, those two items swapped order between the moments.",
    "Read the value labels at each end for the exact numbers.",
    "Hover a line for its name and both values.",
  ],
  story: {
    title: "Nine regions, one quiet disaster",
    paragraphs: [
      "Calum was the finance partner for a software company that sold subscriptions in nine regions, and the quarterly review had a ritual. A slide showed a table of annual recurring revenue by region, last quarter next to this quarter, and the room would murmur approvingly at the totals.",
      "The totals were good. The total is almost always good, which is why a total is a dangerous thing to put in front of people who will go home satisfied.",
      "Calum had a suspicion, the kind that sits at the back of the head like a stone in a shoe. The table would not confirm it, because tables do not show change. They show numbers and ask you to compute.",
      "He took the same eighteen figures and drew them with `<Slope>`, last quarter on the left, this quarter on the right, with the default colouring by direction. Seven of the nine lines pointed gently upward in green. Two pointed down in red, and one of those, the Nordics, had been the second largest region and was now fourth.",
      "The chart put the problem on screen without a word of commentary. People in the room who had been nodding at the total stopped nodding. The Nordics line crossed two others on its way down, and every person present could see it.",
      "It turned out that a reseller contract had lapsed in the second month and no one had flagged it, because the region's growth in other accounts had masked the loss in the total. The contract was renewed within three weeks, and the Nordics recovered about 1.4 million of the 2.1 million they had lost.",
      "A table says here are the numbers. A slope chart says here is who is in trouble.",
    ],
  },
  pitfalls: [
    "**Too many lines.** Past twenty items the labels collide and the picture is a hairball. Filter to the items that matter, or use a different chart.",
    "**Green means good, whatever the metric.** If a rise is bad (churn, defects, cost), set `color_by_change={false}`, or the colours will mislead.",
    "**Comparing slopes across items.** A steep line for a small item is a big percentage and a small absolute change. The y axis is shared and linear, so steepness is absolute.",
    "**Two points do not make a trend.** The line between them is a convenience. It does not say what happened in between.",
  ],
  related: [
    { slug: "bump", why: "When there are many periods and the order of the items matters more than their values." },
    { slug: "dumbbell", why: "When you have many items and want to rank by level and measure the gap on a single axis." },
    { slug: "time-series", why: "When the in-between values exist and you want to show them." },
  ],
  media: [
    { kind: "youtube", id: "_S6X_LUcGLM", title: "Slope Graphs: 3 Ways in 35 Seconds | The Weekly Chart  #inforiver", channel: "Kavita Behera", note: "A 35-second look at slope graphs built with another tool (Inforiver), useful for the before/after comparison idea." },
  ],
}
