import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A bar chart of set intersections, with a dot matrix underneath saying which sets each bar belongs to.",
  intro: [
    "Put five circles in a Venn diagram and you get something a florist would be proud of and a statistician would send back. There are 31 ways to belong to some of five sets, the circles can only draw a handful of them well, and the labels end up wedged into slivers the width of a hair. The Venn diagram is a lovely idea that scales to about three.",
    "[UpSet](https://upset.app/), introduced by Alexander Lex, Nils Gehlenborg and colleagues in a 2014 paper, throws out the circles and keeps the idea. Every combination of sets that actually occurs becomes a **column**. The bar on top says how many items are in exactly that combination. Under it, a column of dots says which sets those are: filled for yes, grey for no, a line joining the filled ones. It looks like a bar chart that is wearing a small spreadsheet, and it works far better than it sounds.",
    "The result is that you can read off the biggest intersections in order, instead of squinting at overlapping lenses. The price is that it is a plot you have to be taught to read once. After that it is hard to go back.",
  ],
  math: [
    {
      heading: "Combinations, not circles",
      body: [
        "Each row of the data lists the sets one item belongs to, joined by a separator (`&` by default), and optionally a value for how many items the row represents. Without one, a row counts as 1. Non-positive or non-finite values and rows with no set names are discarded. The rows are collapsed to exact combinations: a bitmask $m$ of memberships and a total $c_m$.",
        "Intersection sizes in UpSet are **exclusive**. The bar for $\\{A, B\\}$ counts only the items that are in $A$ and $B$ and in nothing else. That is why the bars add up to the whole data set, and why they differ from the pairwise overlap a Venn diagram would show.",
      ],
      tex: [
        "N = \\sum_m c_m, \\qquad \\text{bar}_m = \\frac{c_m}{\\max_{m'} c_{m'}}\\,H",
      ],
    },
    {
      heading: "The set bars",
      body: [
        "The horizontal bars on the left show the size of each set. Unlike the columns, a set size counts everything in the set, including the items it shares with others.",
        "The longest set bar takes the full width of that panel and the rest are drawn relative to it.",
      ],
      tex: ["|S_i| = \\sum_{m \\,\\ni\\, i} c_m"],
    },
    {
      heading: "Which columns are drawn",
      body: [
        "The columns are sorted by size, largest first. Combinations smaller than `min_size` (default 0) are removed first, then only the top `max_intersections` (default 20) survive. A plot with 5 sets can have 31 columns, and past about 20 the dots become too small to read. The cut-off is a readability setting, not a statistical one.",
        "The matrix has one row per set in order of first appearance, with alternating stripes. Each column draws a line from its first to its last filled dot, so combinations that skip a row are still easy to follow. At most 64 sets are supported.",
      ],
    },
  ],
  use: [
    "You have **four or more sets** and you want to know which combinations are common and which are rare.",
    "You care about exact sizes. Every bar has a number and a common baseline, so comparing 412 to 389 is something the eye can actually do.",
    "The data is genomic, survey, tag or feature-flag data, where items belong to many categories at once and the interesting question is which categories travel together.",
    "You want to see that most items belong to just one set (a very tall first few bars, then a long tail) or the opposite (a lot of items in many sets at once).",
  ],
  avoid: [
    "You have two or three sets and the audience has never seen an UpSet plot. [Venn](/charts/venn) answers the same question with no tutorial.",
    "Hundreds of combinations all matter. You will only see the top 20 by default and it is easy to make the picture worse by raising the limit.",
    "You need to see containment (every item in A is also in B). A [sunburst](/charts/sunburst) or tree shows nesting better than dots do.",
  ],
  read: [
    "Start at the top bars and read left to right: columns are sorted from the biggest combination to the smallest.",
    "For any bar, look straight down to the dots. The filled dots, joined by a line, are the sets whose intersection this bar counts, and nothing else.",
    "Use the bars on the left to see which sets are big. A big set whose members mostly sit in single-set columns is a set that stands on its own.",
    "Hover a column for the exact size and its share of all items. Hover a left bar for the full size of that set.",
  ],
  story: {
    title: "Seven flags and a bad Tuesday",
    paragraphs: [
      "Marcus was a platform engineer at a payments company called Ledgerly, and he was in charge of feature flags. There were seven of them guarding the new checkout: express-pay, saved-cards, a currency thing, three experiments nobody remembered starting, and one called `temp_do_not_remove` that had been there for three years.",
      "On a Tuesday, checkout errors rose to four times the normal rate for forty minutes. By the time anybody looked, it had fixed itself, which is the least helpful way for an incident to end.",
      "The question for the post-mortem was simple to ask and annoying to answer: which combinations of flags had the failing users actually had on? Seven flags have 127 possible combinations, and nobody wanted to open 127 dashboards. A Venn diagram of seven circles was never going to survive the meeting.",
      "Marcus exported the failed sessions with the flags they had, one row per session and the flags joined by an ampersand, and passed it to `<UpSet>` in bui-charts. He left every option at its default but the title.",
      "The tall bars on the left were the ones he expected: express-pay alone, saved-cards alone, the two together. But the third from the right was a short column of 61 sessions with the dots lit in rows two, five and seven, and the line between them was the first thing anyone had pointed at all afternoon. Two of the experiments and the flag nobody dared remove, all on together.",
      "That combination existed on exactly one day of the year: the day the three experiments' schedules overlapped. They had never been tested together because they had never overlapped before. One of them was turned off that afternoon. The error rate was back to normal by five, and the other two experiments were rescheduled to avoid each other.",
      "Marcus has since added a column to the rollout checklist: the UpSet plot of who is in what. He says it is the only chart in the company where the most useful part is the part with the least ink.",
    ],
  },
  pitfalls: [
    "**Exclusive versus total.** A column counts items in exactly that combination. If you add the \"A and B\" bar to the \"A, B and C\" bar you get the pairwise overlap, but no bar shows that on its own.",
    "**The 20-column default.** Combinations beyond `max_intersections` are not drawn, so the visible bars may not sum to the total. Raise the limit or filter with `min_size` deliberately.",
    "**Sets with the separator in the name.** `&` is the default separator, so a set called `R&D` is read as two sets. Pass `separator`.",
    "**Counting items versus counting rows.** With no `value`, each row counts as 1. If your rows are already aggregated (one row per combination with a count), pass the count as `value` or every bar will be 1.",
    "**Row order is not meaningful.** Sets appear in the order they were first seen in the data, not by size. If you want the biggest set on top, sort your input.",
  ],
  related: [
    { slug: "venn", why: "For two or three sets, where overlapping circles are easier to read than dots." },
    { slug: "sankey-matrix", why: "When you also want to show how each group splits across a second grouping, not just how groups overlap." },
    { slug: "mekko", why: "When groups are exclusive and you want their sizes and make-up in one picture." },
  ],
  media: [
    { kind: "youtube", id: "n9MRCZxJOfk", title: "UpSet Plots + R Demo", channel: "math et al", note: "Introduces UpSet plots as an alternative to Venn diagrams, with an R demo." },
    { kind: "youtube", id: "Rdu0S8BPWQk", title: "UpSet plot. What it is and how to make it.", channel: "Scientific Illustrations", note: "Explains what an UpSet plot is and how to read and make one." },
  ],
}
