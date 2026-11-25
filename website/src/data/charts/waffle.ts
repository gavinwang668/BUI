import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A grid of squares, filled in order, so a share of a whole is something you can count.",
  intro: [
    "Tell a room that 37% of its users are on mobile and the room nods. Show it a hundred squares with thirty-seven of them coloured in and the room *sees* it, because the human visual system is better at counting small things than at judging the angle of a pie slice. That is the whole case for the waffle chart.",
    "It is the pie chart's friendlier cousin, named for the grid it looks like on the plate, and it has become a favourite of infographic designers and newsrooms. The classic form is ten rows by ten columns, so every square is exactly one percent, and the reader never needs a ruler or a calculator. It also survives the thing that quietly destroys pie charts, which is having an audience that cannot judge angles but can count.",
    "It does have one limit worth knowing before you start: a waffle is a part-to-whole picture of a single whole. It shows you what share each category has, and nothing else. For that job it is hard to beat, for anything harder, it will politely stay quiet.",
  ],
  math: [
    {
      heading: "From values to squares",
      body: [
        "You pass one row per category, each with a name and a value $v_i$. Only finite, positive values count; anything else gets zero squares. With a grid of $R$ rows and $C$ columns (default $10 \\times 10$) there are $M = RC$ squares to hand out, and each category's exact share of them is proportional to its value.",
        "Squares are whole objects, so the exact shares have to be rounded, and rounding each one on its own makes the total drift: three equal categories on 100 squares would each get 33, and a square would be left over. The chart uses the [largest-remainder method](https://en.wikipedia.org/wiki/Largest_remainder_method).",
      ],
      tex: ["e_i = M\\,\\frac{v_i}{\\sum_j v_j}"],
    },
    {
      heading: "Largest remainder",
      body: [
        "Every category first gets $\\lfloor e_i \\rfloor$ squares, the whole part of its share. The squares that are left, $M - \\sum_i \\lfloor e_i \\rfloor$, are handed out one each to the categories with the largest fractional parts $e_i - \\lfloor e_i \\rfloor$, in descending order. Only categories with a positive value are eligible.",
        "The result always adds up to exactly $M$ squares, and no category is ever more than one square away from its exact share. For three equal categories on 100 squares the counts are 34, 33 and 33, and which one gets the extra is decided by the order of the data. If every value is zero or missing, nothing is filled.",
      ],
      tex: ["n_i = \\lfloor e_i \\rfloor + \\mathbb{1}\\bigl[\\, i \\text{ is among the } r \\text{ largest } e_j - \\lfloor e_j \\rfloor \\bigr], \\qquad r = M - \\sum_j \\lfloor e_j \\rfloor"],
    },
    {
      heading: "Layout",
      body: [
        "Categories are laid out in the order of your data, filling the grid row by row from the top left: the first category takes the first $n_1$ squares, the second takes the next $n_2$, and so on. Squares not given to anybody (when there is nothing to show) are left empty. The `gap` (default 0.12) is the space between squares as a share of a square, clamped to 0 through 0.9.",
        "This means that the order you pass is the order you read. Sort largest first and the biggest block anchors the top left. Leave the order alone if it has a meaning, such as a funnel stage or a time period.",
      ],
    },
  ],
  use: [
    "You want to show a handful of shares of one whole and have people take away the number: 12% of orders, 37% on mobile.",
    "The audience is general, or in a hurry, or both. Counting squares needs no training.",
    "You want a percentage to feel like people. A grid of a hundred can stand for a hundred users, and that is a different message from a slice.",
    "You have between two and six categories. That is where colours stay distinguishable and the blocks stay chunky.",
  ],
  avoid: [
    "Differences between shares are small. Two blocks of 31 and 33 squares look alike, and the chart cannot help. A sorted bar chart is sharper.",
    "There are many categories. Past about six, colours blur and tiny categories shrink to one or two squares that nobody can find.",
    "You need to compare shares across several totals. The chart shows one whole at a time. Use a [Mekko](/charts/mekko) when the size of the whole varies too.",
    "You need exactness beyond a square. At 10 by 10 the resolution is 1%. A share of 0.4% rounds to zero or to one full square, depending on the leftovers.",
  ],
  read: [
    "Count squares of one colour. On the default 10 by 10 grid, each square is 1% of the whole.",
    "Read the grid in reading order: the first category in your data starts at the top left and the later ones follow.",
    "Compare blocks by how many rows they fill, not by area impressions. A block of 20 squares is two rows, easy to see.",
    "Hover a square for its category name, and use the legend to switch categories off.",
  ],
  story: {
    title: "A hundred customers, give or take",
    paragraphs: [
      "Hanna led the customer research team at an online bookshop that was famous within the company for a monthly all-hands in which one slide, one slide only, had to answer \"who actually buys from us?\". Her predecessor had used a pie chart with seven slices, and nobody had ever been able to say what the fifth one was.",
      "The data was good. A survey of 2,300 customers sorted them into how they usually bought: 41% for themselves, 23% as gifts, 17% for a book club, 11% for school or work, 6% for a child, and 2% who answered \"other\" and, in the comments, \"it is complicated\".",
      "The problem with these numbers was not accuracy. It was that nobody remembered them. The finance director had once recalled the \"gifts\" slice as \"about half\", and that went into a planning document, where it lived for a quarter.",
      "What she wanted was a picture where 41% looked like forty-one of something, and where a person could look away, look back, and still count. She also wanted the 2% to be visible without a magnifier and to be honest about being small.",
      "She built the slide with a `<Waffle>` from bui-charts: name, value, the survey shares, the default 10 by 10 grid. The component rounded 41, 23, 17, 11, 6 and 2 to squares that totalled exactly one hundred. The 2% sat at the bottom right as two squares that people noticed and mentioned at the end of the meeting.",
      "The slide got the first laughs a market-research presentation in that building had ever had. Someone counted the gift squares aloud and got 23 on the first try, and a person from logistics said he finally knew what to do with the gift-wrap budget. In the next planning round the \"about half\" had become \"23%\", and the wrapping line was cut by a third.",
      "The finance director has asked for every share-of-whole slide to be a waffle since. He has not asked about the two squares, but he looks at them.",
    ],
  },
  pitfalls: [
    "**Rounding is visible.** The squares sum to exactly the grid, but a category's share is rounded to the nearest square. On a 10 by 10 grid, 12.4% and 11.6% both become 12 squares.",
    "**Tiny categories.** A share below half a square can end up with zero squares or with one, depending on how the leftovers fall. Group tiny categories into \"Other\" so the chart is not deciding for you.",
    "**Order carries a message.** Blocks fill from the top left in data order, and the eye treats the first as the most important. Sort on purpose.",
    "**A waffle is not a time series.** Two waffles side by side for two periods are fine for a quick before-and-after, for more than two, switch to a chart whose axis is time.",
  ],
  related: [
    { slug: "mekko", why: "When the whole is not one thing: columns of different sizes, each split into shares." },
    { slug: "warped-waffle", why: "When you want the same count-the-squares feeling in a shape that is not a plain grid." },
    { slug: "lollipop", why: "When comparing the exact sizes of a few categories matters more than the count." },
  ],
  media: [
    { kind: "youtube", id: "jqH8T8Aezd0", title: "Easy Excel Waffle Charts..Better than Pies and Doughnuts!", channel: "MyOnlineTrainingHub", note: "Makes the case for waffles over pies and doughnuts, and shows the 100-square grid in practice." },
  ],
}
