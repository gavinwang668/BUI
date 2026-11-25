import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Bars sorted from biggest to smallest, with a running total that shows where the problem really lives.",
  intro: [
    "Every team has a list of problems and not enough time. The usual response is to work through them in the order they were reported, which is an excellent way to spend a quarter fixing the third most common typo. A Pareto chart is the cure: it sorts the problems by how much they cost, and then shows you how quickly the pile adds up.",
    "The name comes from [Vilfredo Pareto](https://en.wikipedia.org/wiki/Vilfredo_Pareto), the Italian economist who noticed that a small share of the population owned most of the land. [Joseph Juran](https://en.wikipedia.org/wiki/Joseph_M._Juran), writing about quality management, borrowed the idea and called it the vital few and the trivial many. The chart is the picture of that sentence. The tallest bars on the left are the vital few. The long, shallow tail on the right is the rest, and it is where most of your list will turn out to live.",
    "The [80/20 rule](https://en.wikipedia.org/wiki/Pareto_principle) is a rule of thumb, not a law. Sometimes it is 70/30 and sometimes 95/5. What the chart actually gives you is the real number for your data, and a dashed line to tell you where to stop reading and start fixing.",
  ],
  math: [
    {
      heading: "Sorting and the cumulative share",
      body: [
        "Take $n$ categories with values $v_1, \\dots, v_n$ (counts, minutes, cost) and sort them in descending order, so $v_{(1)} \\ge v_{(2)} \\ge \\dots \\ge v_{(n)}$. Write $T$ for the sum of the finite values. The chart draws one bar for each sorted value, and a line for the cumulative share of the total after each bar, as a percentage.",
        "Negative values are counted as zero in the running sum, so the line can never go down, but they still count in the total $T$. If $T$ is not positive, the line is drawn at 0.",
      ],
      tex: [
        "c_k = 100 \\cdot \\frac{\\sum_{i=1}^{k} \\max\\bigl(v_{(i)},\\,0\\bigr)}{T}, \\qquad T = \\sum_i v_i",
      ],
    },
    {
      heading: "Two axes, one chart",
      body: [
        "The bars use the left axis in the units of your data. The cumulative line uses a second axis on the right, fixed from 0 to 100 and formatted as a percentage, with its gridlines hidden so it does not compete with the bars. Each cumulative point is marked with a dot, so you can see where the steps are.",
        "A dashed horizontal line at the `threshold` (80 by default) crosses the cumulative line at the point where the sorted causes have covered that share of the total. The first $k$ with $c_k \\ge \\text{threshold}$ is the count of causes you need to fix to remove that share of the problem.",
      ],
      tex: [
        "k^{*} = \\min\\{\\,k : c_k \\ge \\text{threshold}\\,\\}",
      ],
    },
    {
      heading: "What the sort hides",
      body: [
        "You can pass the data in any order, because the chart sorts it. Ties are kept in input order. The x axis has one slot per category, with names written below, and the legend lists the bars, the cumulative line and the threshold line. Hovering a category shows its value and the cumulative share at that point.",
      ],
    },
  ],
  use: [
    "You have a list of causes, defects, complaints or costs, and need to decide which to attack first.",
    "You want to show, in one picture, that most of the damage comes from a few causes, to people who would prefer it to be spread evenly.",
    "The categories are discrete and have a meaningful count or cost: defect types, ticket categories, failure modes.",
    "You want a stopping rule: the 80% line says when the next bar is no longer worth the meeting.",
  ],
  avoid: [
    "The categories are ordered by something natural, such as age bands or months. Sorting them by size destroys the order. Use a [Histogram](/charts/histogram) or a bar chart.",
    "There are only two or three categories. A sorted line and a threshold add nothing a plain bar does not.",
    "The values are not additive. Averages and rates cannot be summed into a cumulative share.",
    "The categories overlap. If one incident can count in three bars, the cumulative line reaches 100% at the wrong place.",
  ],
  read: [
    "Read the bars left to right. The tallest is the biggest single cause. The drop between neighbours tells you how dominant it is.",
    "Follow the cumulative line. A steep climb at the start means a few causes dominate. A straight diagonal means they are all about equal and there is no vital few.",
    "Find where the line crosses the dashed threshold. Everything to the left is your priority list.",
    "Hover a bar to see its name, value and the cumulative share at that point.",
  ],
  story: {
    title: "Forty-one kinds of wrong",
    paragraphs: [
      "Oyelaran was the quality lead at a company that made printed circuit boards, and he had a problem in the form of a number: 6.4% of boards were failing final inspection. The target was under 2%. His manager had said, kindly, that he would like to see it improve before the end of the year, which is a sentence that has ended many a career in manufacturing.",
      "The inspectors logged every rejection with a defect code, and there were 41 codes: solder bridges, cold joints, lifted pads, misaligned vias, scratches and a category called \"other\" that had, over the years, become the second largest. Every Monday there was a meeting in which each code was discussed, in code order, for about four minutes.",
      "What he needed was to know which of the 41 were worth the discussion. Not all of them could be equally to blame, and if only a handful were, the meeting could stop pretending otherwise.",
      "He exported three months of rejections, about 2,300 of them, and passed the counts per code into `<Pareto>` from bui-charts. The data was in code order and the chart sorted it. He left the threshold at 80.",
      "The cumulative line crossed the dashed line at the fifth bar. Five of the 41 codes accounted for 81% of the rejects: two kinds of solder bridge, cold joints on one connector, and two reflow-oven defects that turned out to be one oven. \"Other\" was in the tail, where it belonged, at 3%.",
      "The Monday meeting now covers five codes, for forty minutes each. Rejects fell to 2.6% in the next quarter after the oven was recalibrated and a stencil was replaced, and to 1.8% in the quarter after.",
      "The remaining 36 codes are reviewed once a month, over lunch. They have not complained.",
    ],
  },
  pitfalls: [
    "**Cost versus count.** Bars measure what you give them. If a rare defect costs a hundred times more per board, chart cost, not count, or the chart will send you after the wrong cause.",
    "**A long tail of tiny categories.** If you have fifty categories with one or two occurrences each, group them as \"Other\" before charting, or the right half of the chart is unreadable.",
    "**The threshold is a convention.** 80% is the default, not a law of nature. Choose one that matches how much of the problem you can afford to ignore.",
    "**Two scales on one chart.** The bars are in your units and the line is in percent. Read them against their own axes, and do not ask where the bar tops meet the line.",
    "**It describes the past.** A Pareto chart shows what has gone wrong. If you fix the top cause, redraw it: the order will change.",
  ],
  related: [
    { slug: "histogram", why: "When the x axis is a continuous quantity and you need its distribution, not a ranked list." },
    { slug: "control-chart", why: "When the question is whether the process is stable over time, not what to fix first." },
    { slug: "ishikawa", why: "When you need to dig into why the top cause happens, rather than rank them." },
  ],
  media: [
    {kind:"youtube",id:"ltBw6kwD3_o",title:"How to use a Pareto chart",channel:"Harvard Online",note:"Short, clear primer on the bars-plus-cumulative-line structure and the 80/20 idea."},
    {kind:"youtube",id:"KS5xVTut4S4",title:"Pareto Charts (1 of 2: Rearranging the data)",channel:"Eddie Woo",note:"Classroom-style walkthrough of building a Pareto chart by sorting the data first."},
  ],
}
