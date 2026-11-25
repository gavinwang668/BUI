import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Words sized by weight and packed outward from the centre along a spiral, so the biggest ones win the middle.",
  intro: [
    "A word cloud is the chart you make when you have 4,000 free-text survey answers and a meeting in twenty minutes. It does not answer a question. It gives you a first impression of the pile, and for that purpose it is brilliant: in under a second you can see that people mostly wrote about \"price\" and \"support\", and you can stop pretending you read all of them.",
    "[Tag clouds](https://en.wikipedia.org/wiki/Tag_cloud) became familiar in the mid-2000s when photo and bookmarking sites started showing the most popular tags in bigger type, and the form went mainstream a few years later with tools like Wordle, which made it easy to produce a poster out of any text. Designers promptly overused it, data people promptly complained, and both groups were a little bit right.",
    "The honest position is somewhere in the middle. A word cloud is a bad way to compare numbers and a good way to show texture. Treat it as the cover page of an analysis, not the analysis itself, and it earns its place.",
  ],
  math: [
    {
      heading: "Weight to font size",
      body: [
        "Every word comes with a positive weight $w$ (usually a count). Words with a weight that is zero, negative or not finite are dropped. The rest are sorted by weight and only the heaviest `max_words` (default 120) are kept. Hidden groups are removed before sizing, so the sizes re-scale to whatever is left.",
        "Font size is not linear in weight. It uses the square root of the position of $w$ between the lightest and heaviest visible words, interpolated between the smallest and largest font size (default 11 and 52 px). The square root keeps the lightest words legible when one term is a hundred times the rest, and it spends more of the size range on the lighter words, where most of the vocabulary lives.",
      ],
      tex: [
        "t = \\sqrt{\\frac{w - w_{\\min}}{w_{\\max} - w_{\\min}}}, \\qquad s = s_{\\min} + (s_{\\max} - s_{\\min})\\,t",
      ],
    },
    {
      heading: "Packing along a spiral",
      body: [
        "The heaviest word is placed first, with its box centred in the chart. Each following word walks an [Archimedean spiral](https://en.wikipedia.org/wiki/Archimedean_spiral) outward from the centre until its bounding box fits without overlapping any placed word and without leaving the canvas. The first position that works is used.",
        "The spiral is stretched horizontally by the aspect ratio $\\alpha$ of the chart (width over height, clamped to 0.5 to 3), so a wide chart fills out sideways rather than leaving the corners empty. The box of a word is its measured text width times 1.08 plus 4 px, by 1.15 times its font size, which is where the little air between words comes from.",
      ],
      tex: [
        "x = c_x + \\alpha\\,2.2\\,\\varphi\\cos\\varphi, \\qquad y = c_y + 2.2\\,\\varphi\\sin\\varphi, \\qquad \\varphi = 0, 0.12, 0.24, \\ldots",
      ],
    },
    {
      heading: "What does not fit, and the fit factor",
      body: [
        "Before placing anything, all sizes are scaled down, if needed, so that the heaviest word is no wider than 85% of the chart. No font goes below 8 px. If a word cannot find a free spot within the search (the spiral stops after 400 radians or once it leaves the chart), it is simply left out, and the smaller words that come after it can still land.",
        "Colour is by group when a group is supplied, so each group gets its own series colour and a legend. Without groups, words cycle through the first eight series colours by rank, which adds variety and carries no meaning. That second case matters: **colour without groups is decoration.**",
      ],
    },
  ],
  use: [
    "You want a **first look at text**: survey answers, support tickets, review titles, search queries. It tells you what the pile is about.",
    "The audience is general and the goal is to make a theme visible, not to make a case. A cloud makes a good cover for a report.",
    "You have pre-counted terms, perhaps with a group such as sentiment or product line, and want to see how different groups use different words.",
  ],
  avoid: [
    "The numbers matter. Longer words look bigger at the same size, and the reader cannot tell 412 from 380. Use a sorted bar chart.",
    "You need to compare two clouds. Sizes are relative to each cloud's own maximum, so a big word in one is not a big word in the other.",
    "Your text has not been cleaned. Raw counts are topped by \"the\", \"and\" and \"of\", which makes for a very accurate and very dull picture.",
    "The order matters. A cloud has no axis and no sequence, so it cannot show change over time.",
  ],
  read: [
    "Look at the biggest words first: the layout puts them in the middle, and they are the most frequent terms.",
    "Notice the second tier. Often the interesting words are two or three sizes down, once the obvious ones are out of the way.",
    "If words are coloured by group, use the legend to see which group a colour is and click to hide a group. The rest re-pack and re-scale.",
    "Hover a word for its exact weight. Do not trust the size to tell you that.",
  ],
  story: {
    title: "The word that was bigger than it looked",
    paragraphs: [
      "Tomasz was a researcher at a mid-sized software company called Brightwire, and he had inherited a spreadsheet. It held 3,812 answers to a single survey question: \"What is the one thing we should fix?\" Nobody had read them. The spreadsheet had been passed from team to team like a mildly radioactive heirloom.",
      "The product director, a patient man, wanted something she could put on the first slide of a planning review without having to explain anything. Tomasz wanted something that would not cost him a week. They agreed that they were both looking for a word cloud and that neither of them was proud of it.",
      "Tomasz tokenised the answers, removed stop words (the earlier draft's biggest word was \"the\"), counted what remained and handed the counts to `<WordCloud>` from bui-charts, with a group for each of the three plans customers were on. The cloud placed \"export\" at the centre in the largest type, followed by \"slow\", \"login\" and \"dashboard\" and then a long ring of one-off terms.",
      "He was about to send it when he noticed that \"export\" was suspiciously fat. It was the heaviest word, yes, with 640 mentions, but the second word, \"slow\", had 610. The sizes used a square-root scale so that the lightest words stayed readable. On the cloud, the two were nearly the same. Without the weights they looked like they were in a different league.",
      "He hovered over each of the top five words and wrote the weights into the slide's caption. It took four minutes, and it changed the conversation. The director's first reaction to the bare cloud had been that export was the obvious priority. After seeing 640 against 610, she asked whether slow and export were the same problem, and it turned out they were: every export took forty seconds.",
      "Fixing the exporter took one sprint and removed both words from the top of the next survey. \"Login\" moved to the centre, where it still lives.",
      "Tomasz says a word cloud is a good way to start a conversation and a poor way to end one, and that the right place for the numbers is a caption somebody will read.",
    ],
  },
  pitfalls: [
    "**Size is a square-root scale.** Doubling a weight does not double the font. Sizes are rank-like, so two words can look similar while differing by a lot.",
    "**Long words look more important.** A word with eleven letters covers more ink than one with three even at the same weight. This is built into the chart type.",
    "**Words can be dropped.** After the 120-word limit (change with `max_words`), and for any word that does not find a free spot. A missing word is not a word with weight zero.",
    "**Stop words and duplicates.** Pass counted, cleaned terms. \"Run\", \"runs\" and \"running\" are three words unless you merge them before counting.",
    "**Colour means nothing without a group.** In ungrouped clouds the colours cycle by rank. Do not read a pattern into them.",
  ],
  related: [
    { slug: "treemap-force", why: "When your terms belong to groups and you want sizes that can actually be compared, with links between them." },
    { slug: "sunburst", why: "When terms form a hierarchy such as category, subcategory and keyword." },
    { slug: "waffle", why: "When a few categories matter and you want the reader to count, not guess." },
  ],
  media: [
    { kind: "youtube", id: "8H0Q_avBq8g", title: "Use a Word Cloud to Explain Your Data", channel: "B.E. with Chris", note: "A practical look at using a word cloud to explain data." },
  ],
}
