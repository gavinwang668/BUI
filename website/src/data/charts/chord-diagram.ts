import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Groups arranged on a circle, with ribbons showing how much flows between each pair, in both directions.",
  intro: [
    "Some questions are not about where things end up but about who is trading with whom. Migration between countries, calls between departments, money between banks, users switching between apps: every group sends to every other, and also receives. A table of this is a square matrix, and a square matrix of twelve by twelve is an exercise in eye strain.",
    "A [chord diagram](https://en.wikipedia.org/wiki/Chord_diagram_%28information_visualization%29) bends that matrix into a circle. Each group gets an arc whose length is how much it sends out. Inside the circle, a ribbon joins each pair of groups, and its width at each end is how much goes in that direction. The ribbons cross in the middle, which looks messy and is the entire point: the picture tells you immediately which pairs are tightly connected and which barely speak.",
    "The form became popular in the genomics world through Circos, and spread from there to anything with a matrix and a designer. It is the one chart here that is both a serious tool for the right data and the most photographed thing in a pitch deck. Use it for the first reason.",
  ],
  math: [
    {
      heading: "The matrix",
      body: [
        "You give the chart a list of $n$ names and a flat, row-major matrix $M$ of $n \\times n$ values, where $M_{ij}$ is the amount that flows **from** group $i$ **to** group $j$. Entries that are zero, negative or not finite are treated as zero. The matrix need not be symmetric, and the diagonal $M_{ii}$ is allowed: it is a flow from a group to itself.",
        "Hiding a group in the legend removes both its row and its column before anything else is computed, and the remaining groups re-scale to fill the full circle.",
      ],
      tex: ["T_i = \\sum_{j} M_{ij}, \\qquad T = \\sum_i T_i"],
    },
    {
      heading: "Arcs and slots",
      body: [
        "The circle is divided among the groups in proportion to what they send out. With $n$ visible groups and a pad angle $\p$ between neighbours (default 2 degrees, capped at half an equal share), the angle per unit of flow is below. Group $i$ owns the arc of $T_i k$ radians.",
        "Each arc is then divided into one **slot** per destination, in the order of the groups. The slot $(i, j)$ is $M_{ij}\\,k$ radians wide and sits inside the arc of group $i$. The sizes are in terms of what a group sends, so the receiving end of a flow lies in the sender's arc, not the receiver's. What a group receives appears as the ends of ribbons that come in from the other arcs.",
      ],
      tex: ["k = \\frac{2\\pi - n\\,p}{T}, \\qquad \\text{slot}(i,j) = \\Bigl[\\theta_i + k\\sum_{l<j} M_{il},\\; \\theta_i + k\\sum_{l \\le j} M_{il}\\Bigr]"],
    },
    {
      heading: "Ribbons",
      body: [
        "For each unordered pair of groups $\\{a, b\\}$ there is one ribbon, joining slot $(a, b)$ to slot $(b, a)$. The two ends can have different widths when the flows are unequal: a wide end and a narrow end is a trade imbalance, drawn as it is. A ribbon with a zero flow in both directions is skipped.",
        "The sides of the ribbon are smooth curves with the circle's centre as the control point, which is what bends them smoothly through the middle. The ribbon takes the colour of whichever direction is larger, at 0.65 opacity by default. A self-flow draws a loop onto the same arc. Hovering a ribbon shows both directions, and the arc tooltips show total out and total in.",
      ],
    },
  ],
  use: [
    "You have a **matrix of flows or relations between the same set of groups**: trade, migration, calls, transfers, co-occurrence.",
    "Flows go in both directions and both matter. The two ends of each ribbon have separate widths.",
    "The number of groups is small, say 4 to 12. Each group is an arc and each pair a ribbon.",
    "You want to see at a glance which group is the hub, which pairs are strong and which groups are isolated.",
  ],
  avoid: [
    "Flows run one way through stages. A [Sankey](/charts/sankey) is clearer, with source on the left and destination on the right.",
    "You need to read exact values. Ribbon widths are hard to compare across the circle. Use a [sankey matrix](/charts/sankey-matrix), which draws the same data as a grid of squares.",
    "You have more than a dozen groups or most cells are filled. The circle becomes a hairball, and a heatmap is better.",
    "Links connect positions inside long segments, as in a genome. [Circos](/charts/circos) is the same idea with positions.",
  ],
  read: [
    "Start with the arcs: a longer arc is a group that sends more.",
    "Pick a group and follow its ribbons out. Where a ribbon is wide at one end and thin at the other, the flow is lopsided.",
    "The ribbon takes the colour of the bigger sender. A ribbon in the colour of group A that ends in group B's arc means A sends more to B than B does to A.",
    "Hover an arc for total out and total in, and a ribbon for both directions. Click a legend entry to hide a group.",
  ],
  story: {
    title: "Who calls whom",
    paragraphs: [
      "Oskar ran operations at a call centre for a utility company called Wattwise, and he had a switchboard problem. Calls came into one of seven departments: billing, outages, new connections, meters, complaints, a team for business accounts and one with the name Miscellaneous, and then were transferred to another department, over and over, until somebody solved the caller's problem or the caller gave up.",
      "The company measured the average call handling time per department and nothing about the transfers. Every department's number looked fine. Callers, who do not experience department averages, were furious.",
      "Oskar suspected the transfers. He had a transfer log of 52,000 rows over a quarter and no sane way to look at it. A table of seven by seven had 49 cells, and he had looked at it for an hour without seeing anything but numbers.",
      "He folded the log into a seven by seven matrix, the row being the department the call came from and the column the department it went to, and passed it to `<ChordDiagram>` from bui-charts, together with the department names. The circle drew itself in a moment.",
      "Most ribbons were thin. But two departments, billing and complaints, were joined by a ribbon that was thick at both ends, which meant calls went back and forth between them. It was the widest ribbon in the diagram. Also Miscellaneous sent 18% of everything it received onwards to Billing, which sent it right back.",
      "The ping-pong turned out to be a refund policy where each department believed the other held the authority to approve it. Oskar wrote the rule down, gave complaints approval up to 200, and the billing-complaints ribbon shrank to a third within a month. Average time to resolve dropped by four minutes, a number he found out the callers were able to feel.",
      "Oskar still keeps the diagram on his office wall. Visitors ask whether it is art. He tells them it is a record of where all the transfers went.",
    ],
  },
  pitfalls: [
    "**Row to column.** The matrix is read as from row to column. If your matrix is the other way round, transpose it, or the ribbon colours will point the wrong way.",
    "**Arc length is what a group sends.** A group that mostly receives has a small arc and many ribbons ending in other arcs. It is not that small groups are unimportant.",
    "**Ribbons are per pair.** Both directions share one ribbon, so you cannot hide one direction. If it is the net flow you care about, compute it first.",
    "**Wrong size, wrong order.** The `matrix` must be exactly $n \\times n$ in row-major order. A matrix of the wrong length silently reads as zeros.",
    "**Order matters for readability.** Groups are laid out in input order. Put related groups next to each other to keep the crossings in the middle down.",
  ],
  related: [
    { slug: "sankey", why: "When flows are one-directional and move through stages." },
    { slug: "sankey-matrix", why: "When you need exact values for every pair of groups." },
    { slug: "arc-diagram", why: "When the order of the nodes has a meaning, such as time, and links are sparse." },
    { slug: "circos", why: "When links join positions along long segments instead of whole groups." },
  ],
  media: [
    { kind: "youtube", id: "gUccbPEv5s8", title: "Chord Diagrams", channel: "Dr. Shahin Rostami", note: "Introduces what chord diagrams show and how to build them." },
    { kind: "youtube", id: "zkcWcZgfM4o", title: "Chord Diagram #OneChartAtATime", channel: "open-source geospatial solutions", note: "Short explainer of one chord diagram example." },
  ],
}
