import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Overlapping circles whose areas and overlaps are sized to the number of items in each set.",
  intro: [
    "Every team has a meeting where somebody draws two circles on a whiteboard and says \"so, who are we building this for?\" The circles are never to scale. The overlap is wherever the marker happened to land. And yet everybody nods, because a Venn diagram is the only chart that has been understood by every adult since roughly the invention of the whiteboard.",
    "John Venn published his version in 1880, in a paper about representing logical propositions diagrammatically, and was gracious enough to call the circles \"Eulerian\" in honour of the man who got there first. The classic textbook picture is a logic device: every possible combination gets a region whether or not anything lives there. This chart does something different. It is an **[area-proportional Euler diagram](https://en.wikipedia.org/wiki/Euler_diagram)**: each circle is sized to the number of items in its set, and each pair is pushed together or apart until the overlap is the right size.",
    "That is a harder trick than it sounds, and it is why you should be a little suspicious of any Venn diagram that looks too tidy. With two sets the picture can be exact. With three it is a good approximation. With four, nature declines to cooperate, and you want a different chart.",
  ],
  math: [
    {
      heading: "Reading rows as set memberships",
      body: [
        "Each row of your data names the sets one item belongs to, joined by a separator (`&` by default), for example `A&B`. An optional value says how many items the row stands for. Without one, every row counts as 1. The rows are folded into exact combinations: a bitmask $m$ of the sets it touches, with a total $c_m$. Rows with a non-positive or non-finite value, or with no set names, are dropped.",
        "The size of a set $i$ is everything that lies inside it, which includes the items it shares with other sets. The size of a pair is everything that lies inside both.",
      ],
      tex: [
        "|S_i| = \\sum_{m \\,\\ni\\, i} c_m, \\qquad |S_i \\cap S_j| = \\sum_{m \\,\\ni\\, i,\\, j} c_m",
      ],
    },
    {
      heading: "Circles with the right area",
      body: [
        "Area is made proportional to size, so the radius of set $i$ is the square root of its size over pi. This is the reason the chart looks lopsided when one set dwarfs another: that is the data, not the layout.",
        "For a pair of circles with radii $a$ and $b$ whose centres are a distance $d$ apart, the overlapping lens has the area below, which the code evaluates directly. The layout then needs the inverse: given the wanted shared area, find $d$. There is no closed form, so it bisects 60 times between $|a-b|$ (one circle inside the other) and $a+b$ (touching). A wanted overlap larger than the smaller circle can hold is clipped to 99.9% of it, and two sets that share nothing are placed with a small gap, $1.08\\,(a+b)$.",
      ],
      tex: [
        "r_i = \\sqrt{|S_i|/\\pi}",
        "A(d) = a^2 \\arccos\\frac{d^2 + a^2 - b^2}{2da} + b^2 \\arccos\\frac{d^2 + b^2 - a^2}{2db} - \\tfrac12\\sqrt{(-d+a+b)(d+a-b)(d-a+b)(d+a+b)}",
      ],
    },
    {
      heading: "Placing three or more circles",
      body: [
        "The first circle goes at the origin and the second on the x axis at its target distance. Each later circle is placed by a brute-force search: a 61 by 61 grid of candidate centres is tried, and the one that minimises the squared error between its actual distances to the circles already placed and the distances their overlaps call for wins.",
        "Three circles can often satisfy all three pairwise distances. They cannot also guarantee the triple overlap. So the region labels are not taken from the formula: the chart samples a 90 by 90 grid over the drawing, works out which circles contain each sample, and writes the true count for each region at the centroid of its samples. Hiding a set in the legend removes its circle and merges its items into the regions that remain.",
      ],
    },
  ],
  use: [
    "You have **two or three sets** and the question is how much they overlap: customers on two plans, papers tagged with three topics, users on web, iOS and Android.",
    "The overlap is the story. When a reader should feel that two groups are mostly the same people, a big shared lens says it faster than a table.",
    "You want a picture for a non-specialist audience. Nobody needs a legend for a Venn diagram, and nobody has ever been confused about what intersection means after seeing one.",
  ],
  avoid: [
    "You have four or more sets. Circles cannot represent every combination of four sets with correct areas, so the picture starts to lie. Use [UpSet](/charts/upset) instead.",
    "You need exact numbers for every combination, including the smallest. A tiny region gets a tiny label or none, and a table or UpSet bar will not hide it.",
    "The sets are nested (every X is a Y). Circles inside circles work but you are then drawing a hierarchy, and a [sunburst](/charts/sunburst) or a plain bar chart will serve better.",
  ],
  read: [
    "Compare circle areas first, not diameters. A circle with twice the radius holds four times as many items.",
    "Each number inside a region is the count of items that belong to exactly those sets, nothing more, so the numbers across the whole diagram add up to the total.",
    "A large overlap relative to the smaller circle means the smaller set is nearly a subset of the larger one.",
    "Hover a circle to see its full size (including the shared parts), how many items belong to that set alone, and its share of everything. Click a legend entry to hide a set.",
  ],
  story: {
    title: "The users who were counted twice",
    paragraphs: [
      "Ingrid ran growth at a small meal-kit company called Forkful, and she had a number that kept getting her in trouble. Forkful had 24,000 customers who had opened the app in the last month, 18,000 who had opened the email newsletter, and 9,000 who had used the referral program. Added together that came to 51,000, which the sales deck cheerfully labelled \"reach\".",
      "Forkful had 30,000 customers in total.",
      "The deck had been through two reviews, one board meeting and a spectacularly confident investor update, and nobody had asked how 51,000 people could come out of a base of 30,000. This is how double counting usually works. The error is not hidden. It simply sits in plain sight, in a table, until somebody draws it.",
      "Ingrid needed a picture that made the overlap impossible to miss, for people who had about ninety seconds each. She loaded the membership rows into `<Venn>` from bui-charts, one row per exact combination with its count: app only, app and email, all three, and so on. The separator was the default, so the whole call was the data, the set column and the count.",
      "The circles came out in the right sizes without her touching anything. The app and email circles were nearly stacked on top of each other, with a big lens in the middle labelled 11,200. The referral circle was small, mostly inside the app circle, and its overlap with email was a thin sliver that her marketing lead stared at for a while and then said, quietly, \"oh.\"",
      "The reach number went from 51,000 to 28,400 unique people in the next deck, and the email programme lost two-thirds of its budget the following quarter: eleven thousand of its readers were already in the app and were already getting its notifications. The referral circle, small as it was, turned out to be the only one pulling in people the other two could not reach, so it got the money.",
      "Ingrid says the most useful feature of the chart is that it is hard to argue with. A table lets you read the number you like. Three overlapping circles show you, uninvited, how much of the room you were counting twice.",
    ],
  },
  pitfalls: [
    "**Three circles, three overlaps, one triple region.** With three sets, the pairwise distances are matched but the centre region is whatever the geometry gives, and it can be larger or smaller than your data suggests. Trust the number printed in it, not the area.",
    "**Counts include the shared items.** The size of a set is its whole circle. If you wanted only the exclusive part, that is the \"only this set\" row in the tooltip.",
    "**Tiny regions lose their labels.** A region with a handful of items out of thousands is a few pixels wide. If it matters, say so in the caption or show it in an UpSet.",
    "**Set names containing the separator.** The default separator is `&`, so a set called `R&D` will be split in two. Pass a different `separator`.",
    "**Nothing counts as zero.** Rows with a value of zero or less, or an empty set list, are ignored. They are not drawn as empty circles.",
  ],
  related: [
    { slug: "upset", why: "For four or more sets, or whenever you need exact sizes for every combination." },
    { slug: "mekko", why: "When the question is how big each group is and what it is made of, rather than how groups overlap." },
    { slug: "sunburst", why: "When one set contains another and the nesting is the point." },
  ],
  media: [
    { kind: "youtube", id: "M6L8mpcZ-HU", title: "Venn Diagram Visualization in Analysis Workspace | Adobe Analytics", channel: "Adobe Analytics", note: "Adobe Analytics walkthrough of a Venn diagram visualization. A different tool but the same chart idea." },
  ],
}
