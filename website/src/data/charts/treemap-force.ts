import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A treemap of groups, with each group's items drawn as circles that cluster by their links.",
  intro: [
    "Two charts are very good at two different things. A treemap tells you how big each group is, by the area of its rectangle, and uses every pixel doing it. A force-directed graph tells you who is connected to whom, by pulling friends together and pushing everyone else away, and uses most of its pixels to look interesting. Wanting both is reasonable and, until recently, meant putting two charts on a slide and hoping.",
    "A treemap force chart puts them together. Groups become rectangles, arranged by the [squarified treemap](https://en.wikipedia.org/wiki/Treemapping) algorithm that Mark Bruls, Kees Huizing and Jarke van Wijk described in 2000, so that the area of each group is its total. Inside each rectangle, the items are circles, sized by value, and links between items pull linked circles toward each other. You see size, membership and connections in one frame.",
    "It is a busy picture and it is meant to be. The rule that keeps it honest is that nothing leaves its rectangle. A circle can be pulled hard towards a friend in another group, but it will stop at the wall, and you can see the strain in the line between them.",
  ],
  math: [
    {
      heading: "Squarified rectangles",
      body: [
        "Each item has a group and a non-negative value $v$ (non-finite values count as zero; hidden groups are excluded). A group's total is the sum of its items, $G_g = \\sum_{i \\in g} v_i$. Groups with a total of zero are not drawn.",
        "Groups are sorted by total, largest first, and packed into the chart with the squarified treemap algorithm: rectangles are added to a row along the shorter side of the free space for as long as doing so improves the worst aspect ratio in the row, then a new row starts. Area scales so that the sum of all rectangle areas is the chart area, so each group's area is its share of the total.",
      ],
      tex: [
        "A_g = \\frac{G_g}{\\sum_h G_h}\\,W H, \\qquad \\text{worst}(R) = \\max\\left(\\frac{s^2 \\, a_{\\max}}{\\Sigma^2},\\; \\frac{\\Sigma^2}{s^2\\, a_{\\min}}\\right)",
      ],
    },
    {
      heading: "Circles inside a pen",
      body: [
        "Inside each group's rectangle there is a pen (the rectangle less a 2 px margin and, if there is room, a title strip). An item's circle has the area of half of the pen shared out by its value, so a circle fills about 50% of the pen at most when it is alone: $r_i = \\sqrt{ (v_i / G_g) \\cdot A_{pen} \\cdot 0.5 / \\pi }$. The radius is clamped to at least 2.5 px and to just under half the pen's shorter side.",
        "Starting positions are spread over a golden-angle spiral (2.399963 radians between successive items) around the pen's centre, so that no two items begin at the same point.",
      ],
    },
    {
      heading: "A small force simulation",
      body: [
        "Links are pairs of item indices with a weight. Self-links and indices out of range are discarded. Each link is a spring whose natural length is the sum of the two radii plus a `distance` (default 14 px). Over `iterations` steps (default 240), each spring pulls its two ends together or pushes them apart toward that length, scaled by $0.3\\,\\alpha\\, k$, where $\\alpha$ falls linearly from 1 to 0 over the run (a cooling schedule) and $k$ is the link strength.",
        "Circles in the same group repel when they overlap, with 1.5 px of clearance. After every step each circle is clamped back into its pen. Links across groups pull circles toward the edge they share, but there is no force between groups, which is why the picture is stable and not a hairball.",
      ],
      tex: ["k = 0.4 + 0.6\\,\\frac{w}{w_{\\max}}, \\qquad \\Delta = 0.3\\,\\alpha\\,k\\,\\frac{d - (r_a + r_b + \\text{distance})}{d}"],
    },
  ],
  use: [
    "You have **items in groups and links between items**, and both the group sizes and the links matter: teams and who depends on whom, departments and who reports to whom, repositories and who imports what.",
    "You want a map where every pixel belongs to something. Treemaps use the whole canvas, and the force layer says something about the structure within each tile.",
    "Cross-group links are the point. A thick line between two circles in different rectangles says there is a dependency across a boundary.",
  ],
  avoid: [
    "Nothing is linked. Without links the circles just sit in their rectangles, and a plain treemap or [sunburst](/charts/sunburst) will do the same job with less noise.",
    "You need exact values. Circle area is hard to judge, which is the reason circles are labelled when they fit.",
    "You have hundreds of items per group. Circles shrink to dots and labels vanish. Aggregate the tail first.",
    "You want to show direction or flow. Links are plain lines with weights and no arrows. Use a [sankey](/charts/sankey) or a [node graph](/charts/node-graph).",
  ],
  read: [
    "Start with the rectangles: the biggest area is the group with the biggest total.",
    "Inside a rectangle, bigger circles are bigger items. Compare within a group before you compare across groups.",
    "Follow the lines. A short thick line is a strong link. A long thin line between rectangles is a weak dependency that crosses a boundary.",
    "Hover a circle for its name, group, value and the number of links it has. Click a legend entry to hide a group and the rest re-pack.",
  ],
  story: {
    title: "The dependency nobody drew",
    paragraphs: [
      "Delphine was an engineering manager at a company called Quillpost, which made email software and a lot of internal services to support it. There were 38 services owned by six teams, and the company's architecture diagram was a PDF from 2021 that showed 26 of them and one cloud.",
      "A reorganisation was coming. Three teams were going to merge, and the question on everyone's mind was which services they would have to take care of and how many of those depended on services owned by people who were not in the merge. Delphine had until Monday and a vague feeling that the answer was \"more than we hope\".",
      "She had two datasets. One was a list of services with owners and a measure of how much traffic each handled. The other was a list of calls between services, with call counts. A treemap would show her what each team owned. A graph would show her the calls. Neither would show her both, and the second question was the one that mattered.",
      "She loaded both into `<TreemapForce>` from bui-charts: services as items, teams as groups, traffic as value, and the call counts as weighted links. The teams became rectangles. Inside each, the services drifted into clusters. Everything stayed inside its team's tile, and the lines ran across the borders.",
      "Most of the lines stayed in their tiles, which was the good news. The bad news was a thick pair of lines leaving the payments team's rectangle and ending in a small circle in the platform team's: an auth service with 14 incoming links and a value so low it was barely visible. It carried almost no traffic and was depended on by everything.",
      "That service was not on the architecture diagram. It was owned by a team of one person who was on parental leave. Delphine moved it, and a second engineer, into the merged team's scope before the reorganisation went through. It was the first item on the Monday agenda and the last thing anyone complained about.",
      "She has since added the chart to the quarterly review. When someone says a service is unimportant, someone else points at the lines.",
    ],
  },
  pitfalls: [
    "**Links are indices.** Links refer to items by their position in the data, so reordering or filtering your input changes what they point to. Build them from the same list you pass in.",
    "**Circle size is within-group.** The radius depends on an item's share of its group and the size of the pen, so a circle in a big group is not comparable to one in a small group by area alone.",
    "**Layout is deterministic but not unique.** The same data always gives the same picture, but a different `iterations` or `distance` produces a different layout. Positions carry no meaning. Only membership and links do.",
    "**Cross-group links are drawn straight.** They may cross other circles. Read the endpoints, not the path.",
    "**Zero-value items are hidden.** An item with a value of zero or less is not drawn, and links to it are dropped.",
  ],
  related: [
    { slug: "node-graph", why: "When the connections are the point and size and grouping are not." },
    { slug: "sunburst", why: "When groups nest inside groups and you want a hierarchy rather than links." },
    { slug: "chord-diagram", why: "When you want flows between groups, not between individual items." },
  ],
  media: [
    { kind: "youtube", id: "fXErUBWZaNQ", title: "Interactive data visualization of the hierarchical data structure - D3.js", channel: "Piotr Paszek", note: "D3.js walkthrough of an interactive hierarchical visualization. A neighbouring technique rather than this exact chart." },
  ],
}
