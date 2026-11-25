import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Shows how each item's rank changes from one period to the next, ignoring the underlying values.",
  intro: [
    "Sometimes the score is not what you care about. In a league table you want to know who is first, who is second, and who has just fallen out of the top three, and you want to know it across a whole season. A line chart of points would show each team's tally rising together, and you would squint at the overlaps to work out the order.",
    "A bump chart throws the scores away and keeps the ranks. The vertical axis is position, with first place at the top. Time runs left to right. Each item is a thick line, and every time two lines swap places they cross, which is the whole drama of a ranking, given its own shape.",
    "It gets its name from the bumps: a line that jumps up a place bumps past another, which bumps down. You can follow the story of one competitor across eight rounds with your finger, which is the thing a table of ranks has never allowed.",
  ],
  math: [
    {
      heading: "From values to ranks",
      body: [
        "The data is long: one row per item per period, with a period value $t$, a name, and a score $v$. The chart collects the distinct names in order of first appearance and the distinct periods in ascending order, then lays the scores on a grid. A missing score leaves a gap.",
        "Within each period the finite scores are sorted and numbered. With `highest_first` on (the default), the largest score gets rank 1. Ties are broken by the order in which the names first appeared, since the sort is stable.",
      ],
      tex: ["r_{i}(t) = 1 + \\#\\{\\, j : v_j(t) > v_i(t) \\,\\} \\quad \\text{(ties broken by first appearance)}"],
    },
    {
      heading: "What gets drawn",
      body: [
        "Each item is a line through its ranks, with a marker at every period. The y axis is reversed so that rank 1 is at the top, and it runs from $0.5$ to $n + 0.5$ for $n$ items, so every integer rank has half a unit of margin. Ticks are whole numbers, and the axis is labelled Rank.",
        "Lines are smoothed by default (`smooth`), which makes a crossing look like a gentle S rather than a kink, but does not move the dots. An item with no score in a period has no rank there, and its line breaks.",
      ],
      tex: ["y \\in [\\,0.5,\\ n + 0.5\\,], \\qquad \\text{axis reversed}"],
    },
  ],
  use: [
    "The order is the story: league tables, popularity charts, sales rankings, benchmark leaderboards.",
    "You have between 3 and about 12 items and 4 or more periods, so crossings have room to happen.",
    "You want to show a climber and a faller on the same canvas without the scale of the scores getting in the way.",
  ],
  avoid: [
    "The size of the gaps matters. Rank 1 and rank 2 may be separated by a hair or a mile, and the chart treats them as one step apart. Use a [time series](/charts/time-series) for that.",
    "There are only two periods. A [slope](/charts/slope) chart says the same thing with less ceremony.",
    "You have dozens of items. The lines braid together and only the top few are readable.",
    "Many items are missing in many periods. Gaps in a rank chart are confusing, since every present item's rank depends on who else was there.",
  ],
  read: [
    "Find the line of the item you care about and follow it left to right. The vertical position is its rank, with first at the top.",
    "Look for steep lines. A steep climb or drop is a big change in position, even if the underlying score barely moved.",
    "Each crossing is a swap between two items. Count them to see how turbulent the ranking was.",
    "Click a legend entry to hide a line and clear the clutter. Hidden items are not removed from the ranking.",
    "Hover a column for every item's rank in that period.",
  ],
  story: {
    title: "Seven weeks in fourth place",
    paragraphs: [
      "Ayesha ran the analytics for a school chess league, a job which paid in sandwiches. Eight teams played a [round-robin](https://en.wikipedia.org/wiki/Round-robin_tournament) over eight weeks, and each Friday the league newsletter published a table of points. The table was complete, correct and read by absolutely nobody.",
      "The coach of the Hillcrest team, a patient man named Mr Obi, had asked her a gentle question: were his kids actually improving, or did it only feel that way because he wanted it to?",
      "Points would not answer that. Every team's points go up every week, because that is what points do. Hillcrest went from 3 to 4 to 6 to 7, and so did everyone else, in slightly different proportions, and a line chart of the totals is eight hopeful diagonals that look like a hair-dryer.",
      "She took the weekly points as long rows, one per team per week, and gave them to `<Bump>`. It ranked the teams within each week by itself and drew them as eight coloured lines, rank 1 at the top.",
      "Hillcrest was easy to find. It started in seventh, crawled to fifth, and then fell to a fourth place that it held, without moving, for seven weeks. The line was a long, steady shelf, which in the context of a school chess league is called consistency.",
      "More useful was what happened above it. The two teams ahead had swapped places three times, and the leader had dropped from first to third and back. Mr Obi realised that his team was not stuck: it had been the most stable team in the league while everyone near the top was in a state of panic. He moved the kids from a rotating board system to a fixed one, and they finished third, which was celebrated with a cake the size of a laptop.",
      "A ranking is a very small story about a very large number of results. This chart gives it back its plot.",
    ],
  },
  pitfalls: [
    "**Rank hides margin.** Being first by one point and first by a hundred look identical. Show the scores somewhere nearby.",
    "**Ties are broken, not shared.** Equal scores get consecutive ranks in the order the names first appeared. If ties matter, add a small tie-breaker to the value yourself.",
    "**Period values must match exactly.** Rows are matched to periods by their numeric x. If one team reports at 3.0 and another at 3.0000001, you get two periods.",
    "**Direction.** The default ranks the highest score as 1. For times or golf scores where lower is better, set `highest_first={false}`.",
    "**Too many items.** Each item needs its own colour from the palette. Beyond about a dozen, colours repeat and the chart becomes guesswork.",
  ],
  related: [
    { slug: "slope", why: "The two-period version: fewer ways to get confused, with real values at both ends." },
    { slug: "time-series", why: "When values, not positions, should carry the story." },
    { slug: "alluvial", why: "When items move between categories rather than between ranks." },
  ],
  media: [
    { kind: "youtube", id: "DkxpAHuvq1o", title: "#Tableau - Bump Chart", channel: "Andy Kriebel", note: "Concise build of a bump chart by a well-known Tableau practitioner. Uses Tableau, not this library." },
    { kind: "youtube", id: "ZTq6Feerr3s", title: "Bump Chart in Tableau in 60 Seconds! 📈 | Track Rankings Over Time #shorts", channel: "Nerds of Data", note: "Very short demo of using a bump chart to track rank over time." },
  ],
}
