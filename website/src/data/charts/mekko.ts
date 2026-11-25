import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A stacked bar where the width of each column carries information too.",
  intro: [
    "Every business question eventually becomes two questions in a trench coat: *how big is it?* and *what is it made of?* A normal bar chart answers the first. A 100% stacked bar answers the second. Ask both at once and you get the classic meeting where somebody has to flip between two slides and the room quietly decides lunch is more interesting.",
    "A Mekko chart (also called a [Marimekko](https://en.wikipedia.org/wiki/Marimekko), after the Finnish fabric whose bold blocks it resembles, and also called a variable-width stacked bar by people who've never had any fun) puts both answers on one canvas. Each column is a category. The **width** of the column is how big that category is. The **height** of each block inside it is that block's share of the category. Which means the **area** of a block is its share of the whole thing, and you get to compare every block in the chart at a glance.",
    "That last sentence is the reason to use it. In a Mekko, big blocks are big because they matter, not because the artist felt generous with the y-axis.",
  ],
  math: [
    {
      heading: "Three probabilities in one picture",
      body: [
        "Say you have a table of non-negative values $v_{ij}$: row $i$ is a column of the chart (a region, say) and $j$ is a segment inside it (a brand). Write the column totals as $T_i$ and the grand total as $T$.",
        "The chart is a picture of a joint distribution, factored the way you'd factor it on a whiteboard: pick a column first, then pick a segment inside it.",
      ],
      tex: [
        "T_i = \\sum_j v_{ij}, \\qquad T = \\sum_i T_i",
        "\\underbrace{\\frac{v_{ij}}{T}}_{\\text{area}} \\;=\\; \\underbrace{\\frac{T_i}{T}}_{\\text{width}} \\times \\underbrace{\\frac{v_{ij}}{T_i}}_{\\text{height}}",
      ],
    },
    {
      heading: "What gets drawn",
      body: [
        "With a drawing width $W$, a drawing height $H$ and $n$ columns separated by a gap $g$, column $i$ gets the width below, and the block for segment $j$ inside it gets the height below. Hidden segments are removed before the totals are computed, so the remaining columns re-scale to fill the chart instead of leaving a hole.",
      ],
      tex: [
        "w_i = \\frac{T_i}{T}\\,\\bigl(W - (n-1)\\,g\\bigr), \\qquad h_{ij} = \\frac{v_{ij}}{T_i}\\,H",
      ],
    },
    {
      heading: "Why area is honest here",
      body: [
        "Area charts usually lie because the eye compares the wrong dimension. Here both dimensions are proportions of something real, so the product is too. The only distortion is perceptual: humans are better at comparing lengths than areas, which is why the chart prints the share inside each block that fits.",
      ],
    },
  ],
  use: [
    "You care about **size and mix at the same time**: market size and market share, budget and where it goes, traffic and where it comes from.",
    "Columns differ a lot in size. If one region is nine times the next, you want the reader to feel that.",
    "You want to pick the one block that deserves attention out of a dozen, and the area tells you which.",
  ],
  avoid: [
    "Columns are all roughly the same size. Then the width carries nothing and a plain 100% stacked bar is easier to read.",
    "You have more than about seven segments. The colours blur, and the top and bottom blocks are the only ones anyone can compare.",
    "Your audience has to read exact values off the chart. Use a table, or a bar chart per column.",
    "Some columns are tiny. A column that is 1% wide gets 1% of the pixels, so its labels cannot fit. That is the chart being truthful, not broken.",
  ],
  read: [
    "Start with the widest column: that is where most of the whole lives.",
    "Look at the mix inside it. Segment order is the same in every column, so you can follow one colour across the chart.",
    "Then look for the odd one: a thin column with a huge block of one colour is a small market you dominate, which is either a strategy or a trap.",
    "Hover any block for its value, its share of the column and its share of the grand total.",
  ],
  story: {
    title: "The 18% that wasn't",
    paragraphs: [
      "Dana ran finance for a regional pizza chain called Crust Pirates, which sold pies, garlic knots and, for reasons nobody could document, a thing called the Plank. Every quarter she presented market share by region to the board, and every quarter the same ten minutes happened.",
      "The deck had a clean 100% stacked bar for each region. Europe: Crust Pirates 34%. Asia: 18%. Africa: 12%. Latin America: 22%. And every quarter the CEO, a man who treated slides like a moral test, would stab a finger at Africa and ask why the company was ignoring a 12% share when it was clearly *so close* to Asia's 18%.",
      "The bars were all the same width. Dana knew Asia was nine times the size of Africa. The chart knew too, but it had been told to keep quiet.",
      "She needed one picture that showed how big each region was *and* what the pie split looked like inside it, with no second slide and no footnote saying \"see appendix\". That is a very specific job description, and it turns out someone had already hired a chart for it.",
      "She rebuilt the slide as a Mekko with `<Mekko>`: one column per region, the width set by regional sales, the blocks split by brand. Same data, same colours, one prop for the segment names. The Asia column swallowed half the slide. The Africa column became a polite sliver.",
      "The CEO opened his mouth, looked at the sliver, and closed it again. Dana said the board meeting went from forty minutes to twelve, which she attributes to the chart and which her colleagues attribute to the fact that the last item was the Plank.",
      "The following quarter the board asked a better question: why was the Asia column so full of *other* brands? That one was worth the forty minutes.",
    ],
  },
  pitfalls: [
    "**Segment order.** Put the segment you most want people to compare at the bottom: the baseline is the only place the eye can measure heights accurately.",
    "**Too many columns.** Past twelve or so, the labels under the columns collide. Group the long tail into \"Other\".",
    "**Zeros and negatives.** A Mekko is a picture of parts of a whole, so negative values are ignored. If you have returns or refunds, show them as their own column.",
    "**Comparing across charts.** Widths are relative to the total of *this* chart. A column that is 20% wide here is not the same size as a 20% column in last quarter's chart unless the totals match.",
  ],
  related: [
    { slug: "waffle", why: "When you only need the mix and want people to count units, not judge areas." },
    { slug: "sunburst", why: "When the categories have sub-categories and sub-sub-categories." },
    { slug: "sankey-matrix", why: "When you also need to see how each row flows into each column, with exact values." },
  ],
  media: [
    { kind: "youtube", id: "bM4IwgsUaKY", title: "Create a Marimekko Chart aka Mekko Chart", channel: "Doug H", note: "A short walkthrough that builds a Mekko from a plain table, which makes the width-times-height idea concrete." },
    { kind: "youtube", id: "iNacUTcGCQ4", title: "Think-Cell Mekko Chart Tutorial (Units & Width Explained)", channel: "Paschal Ugwu Solutions", note: "Spends its time on what the column widths mean and how units change the reading, the part people most often get wrong." },
  ],
}
