import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Services as circles, calls as arrows, and the health of each one drawn around its edge.",
  intro: [
    "Every architecture diagram begins life as a tidy drawing on a whiteboard and ends as a lie. Somebody adds a queue, somebody else adds a cache, a third person adds a service called \"misc\", and nobody updates the drawing. A node graph is the architecture diagram that cannot lie, because it is built from what the system actually did.",
    "Each node is a service, a stage or a dependency. Each edge is a call between two of them. The twist, from [Grafana's Node graph panel](https://grafana.com/docs/grafana/latest/panels-visualizations/visualizations/node-graph/) which this chart ports, is that nodes carry numbers. Two stats sit inside the circle, a title below, and the outline is split into coloured arcs, so a node can say \"98% success, 2% errors\" without being clicked.",
    "That is the whole appeal. A table of services answers \"what is the error rate of checkout?\". A node graph answers \"which of the services that checkout depends on is the one on fire?\", and gives you the other thirty at the same time.",
  ],
  math: [
    {
      heading: "Layered layout",
      body: [
        "The default layout turns the graph into columns, so edges point rightward wherever they can. First, cycles are broken: a depth-first search reverses any edge that points back to a node on the current path, and self-loops are ignored. On the resulting acyclic graph each node gets a layer equal to the length of the longest path leading into it.",
        "Here $L(v)$ is the layer of node $v$ and $\\mathrm{pred}(v)$ its predecessors. Edges that span more than one layer get invisible dummy nodes in the layers between, so they take part in ordering.",
      ],
      tex: [
        "L(v) = \\max_{u \\in \\mathrm{pred}(v)} \\bigl(L(u) + 1\\bigr), \\qquad L(v) = 0 \\text{ if no predecessor}",
      ],
    },
    {
      heading: "Reducing crossings and placing nodes",
      body: [
        "Within each layer, the order of nodes is chosen by 12 alternating sweeps. On a downward sweep every node is sorted by the mean position of its neighbours in the layer above. On an upward sweep, by those below. This [barycentre heuristic](https://en.wikipedia.org/wiki/Layered_graph_drawing) is the classic cheap way to cut edge crossings. It does not find the minimum, and does not claim to.",
        "Vertical coordinates start evenly spaced at a step of 140 px (a 100 px circle plus a 40 px gap) and are then pulled toward the mean of their neighbours for 8 passes, with the constraint that nodes in a layer cannot overlap. Columns are 160 px apart. The result is centred on the origin.",
      ],
      tex: [
        "x_v = 160\\,L(v), \\qquad \\mathrm{key}(v) = \\frac{1}{|N(v)|}\\sum_{u \\in N(v)} \\mathrm{order}(u)",
      ],
    },
    {
      heading: "Force layout, grid and arcs",
      body: [
        "`Force` runs a physical simulation for 300 ticks with Grafana's [d3-force](https://d3js.org/d3-force) settings: links pull their endpoints toward a rest length of 150 px with strength 0.5, a collision radius of 100 px pushes overlapping nodes apart, and a velocity decay of 0.6 damps the motion. Roots and their direct children start on a coarse tree and keep their horizontal position, which is why force layouts here still flow left to right. `Grid` ignores edges and fills rows in data order.",
        "A node's outline is divided by its `arcs` values $a_1, \\dots, a_k$, clockwise from the top. If any value is 1 or more, the whole outline takes that colour. Otherwise each non-zero $a_i$ gets that share of the circle and anything beyond 100% in total is cut off.",
      ],
      tex: [
        "\\text{sweep}_i = 360^\\circ \\cdot a_i, \\qquad \\textstyle\\sum_i a_i \\le 1",
      ],
    },
  ],
  use: [
    "You want to see how a set of services depend on each other, with health or load on each, in one view.",
    "The question is \"where is the problem?\": a red arc on a node, and the edges that touch it, point at the culprit.",
    "The data is a graph: pipeline stages, dependency trees, data lineage, anything with a source and a target.",
    "You want a starting map for an incident, where hovering a node dims everything except it and its neighbours.",
  ],
  avoid: [
    "The data is about one request in time. Use [Traces](/charts/traces), where the x axis is a clock.",
    "You need to compare flow volumes accurately. Edge thickness is a hint. A [Sankey](/charts/sankey) gives width proportional to flow.",
    "The graph has thousands of nodes and no obvious roots. The chart caps what it draws (200 by default) and the rest will be collapsed into markers.",
    "The relationships are dense and between a small set of groups. A [Chord Diagram](/charts/chord-diagram) shows that better.",
  ],
  read: [
    "Start at the left in the layered layout: roots (nodes nothing calls) are there, and calls go rightward.",
    "Scan the outlines for colour. An arc split between success and error colours tells you the share of each before you read a number.",
    "Hover a node: it and its neighbours stay bright, the rest dim, and the stats on the connected edges appear. Hover an edge for its own stats.",
    "Click a node or edge for a popover with its stats and extra rows. Drag the background to pan, use + and - to zoom (hold Ctrl with the wheel), and the layout buttons to try Force or Grid.",
  ],
  story: {
    title: "Nobody calls the thing that everybody calls",
    paragraphs: [
      "Priya joined a logistics company as its fourth platform engineer, inheriting a system of forty-odd services and a wiki page titled \"Architecture (current)\" that had last been edited by someone who had since left to open a bakery. Her first week was supposed to be reading. She spent it asking people what depended on what, and getting forty-odd slightly different answers.",
      "The question became urgent in her third week, when the parcel-routing service started returning errors for about three percent of requests. Three percent is the dangerous number: too small to page anyone, large enough that somebody's delivery is a week late.",
      "She did not need another dashboard of per-service charts. She needed the shape of the failure: which of the forty services were affected, in what order, and which one they all seemed to lean on. That is a question about edges, not values.",
      "She exported the service-to-service call counts and error rates from the metrics store and drew them with `<NodeGraph>` in a small desktop tool: nodes with the request rate inside, arcs for success and error share, edges labelled with latency. The layered layout put the public API on the left and the databases on the right, and made the shape plain at once.",
      "Three services had a faint red arc. Every edge feeding them came from the routing service, and the routing service had one edge, thick and dark, into a node called `geocoder-legacy` whose outline was almost half red. It was the only service in the graph that the wiki did not mention.",
      "The geocoder was running on a single instance and timing out under load. Adding two replicas dropped routing errors from 3.1% to 0.2% by the afternoon. The wiki page was updated, and the bakery was informed that the old owner was off the hook.",
      "Priya's graph now lives on a side monitor. It has, as far as she can tell, the same effect as a smoke detector: nobody looks at it until it is the only thing that matters.",
    ],
  },
  pitfalls: [
    "**Layout is not meaning.** In the layered layout, position along the x axis is the number of hops, not time or size. In the force layout, distance is only loosely related to anything.",
    "**Cycles are drawn, but bent.** The layered layout reverses back-edges for placement only. A call loop will still be an arrow in the right direction, but it may cross the whole graph to get there.",
    "**Arcs that sum past 1.** Each value is a share, and shares past 100% in total are cut off. If your two arcs are \"success\" and \"errors\", make sure they add to at most 1.",
    "**The node limit.** With more than `node_limit` nodes, only those nearest the roots are laid out and the rest become \"N nodes\" markers. Check for them before concluding a service is not there.",
    "**Dangling edges.** An edge whose source or target id is not among the nodes is silently left out.",
  ],
  related: [
    { slug: "traces", why: "When you need one request in time rather than the whole map." },
    { slug: "sankey", why: "When edge widths must be proportional to volume." },
    { slug: "chord-diagram", why: "When a few groups are densely connected and you want to compare flows between them." },
  ],
  media: [
    {kind:"youtube",id:"g-4ODB-c4ac",title:"Force directed network graph - D3.js interactive data visualization",channel:"Piotr Paszek",note:"Shows an interactive force-directed node-link graph. Made with D3 rather than this library."},
  ],
}
