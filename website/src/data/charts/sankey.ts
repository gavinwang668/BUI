import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Nodes joined by ribbons whose width is the amount that flows between them.",
  intro: [
    "A pie chart tells you where the money is. A Sankey diagram tells you where the money went. That is a different and far more useful question, and the answer is almost always more embarrassing: income comes in on the left, fans out through departments, and ends on the right in a node labelled \"Other\" that is bigger than three of the departments.",
    "The form is named for Captain [Matthew Sankey](https://en.wikipedia.org/wiki/Matthew_Henry_Phineas_Riall_Sankey), who used it in 1898 to show how much of a steam engine's energy was useful and how much escaped as heat. It had been in use before him: [Charles Minard](https://en.wikipedia.org/wiki/Charles_Joseph_Minard)'s flow maps of the 1840s and his famous 1869 chart of Napoleon's march on Moscow draw a band whose width is the size of an army. The name stuck to Sankey anyway, which is a consolation to anybody who has ever had a diagram named after the wrong person.",
    "What makes the chart good is that width is conserved. What enters a node leaves it, so a reader can follow a ribbon from source to destination and see exactly how much arrived. What makes it hard is that it is a layout problem, and a bad layout turns it into noodles.",
  ],
  math: [
    {
      heading: "From rows to a flow graph",
      body: [
        "Each row gives a source, a target and a value. Nodes are identified by name, so \"Sales\" as a source and \"Sales\" as a target are the same node. Rows with the same source and target are summed into one link. Rows where the source equals the target, or whose value is zero, negative or not finite, are dropped.",
        "A node's throughput is the larger of what flows in and what flows out. A balanced node has both equal. An unbalanced one, such as a pure source or a leaky node, takes the bigger side.",
      ],
      tex: ["f(n) = \\max\\Bigl(\\sum_{e\\to n} v_e,\\; \\sum_{n\\to e} v_e\\Bigr)"],
    },
    {
      heading: "Layers and scale",
      body: [
        "Nodes are put in columns by their distance from the sources. The chart uses the justified layout: nodes without outgoing links are pushed to the last column, so that every sink lines up on the right edge. The flows must be acyclic: a loop (A to B to A) cannot be put in columns, and the layout will not produce a diagram.",
        "In every column the nodes are stacked with a fixed padding $p$ (default 12 px). A single pixels-per-unit factor is chosen so that the most crowded column just fits in the height $H$. Using one factor for the whole chart is what makes widths comparable everywhere: a ribbon of 10 units is the same thickness at the left edge as at the right.",
      ],
      tex: [
        "k = \\min_{c}\\ \\frac{H - (n_c - 1)\\,p}{\\sum_{n \\in c} f(n)}, \\qquad h_n = k\\,f(n), \\qquad w_e = k\\,v_e",
      ],
    },
    {
      heading: "Relaxation and drawing",
      body: [
        "Initially nodes are simply stacked. Then six relaxation passes alternately sweep right to left and left to right, moving every node toward the weighted centre of the nodes it connects to and resolving overlaps afterwards, with the strength of each pass growing so that the layout settles. This is the same scheme used by the [d3-sankey](https://github.com/d3/d3-sankey) library, and it is why the diagram is not unique: different relaxation settings give different, equally valid pictures.",
        "A ribbon runs from the right edge of its source to the left edge of its target as a smoothly eased band, its width given by $w_e$, with a minimum of one pixel so that tiny flows are still visible. Node bars are 12 px wide by default, ribbons are drawn at 0.4 opacity in the colour of their source, and hovering a node or a ribbon fades everything not connected to it to 35% of that.",
      ],
    },
  ],
  use: [
    "You want to show **where a quantity goes**: money through a budget, energy through a plant, users through a product, visitors from channels to pages.",
    "Loss matters. The difference between what enters and leaves each stage is the point, so the widths need to be comparable along the flow.",
    "The flow is **one-directional**: it starts at sources and ends at sinks, with no loops.",
    "You want to find the one big ribbon. A Sankey is excellent at making one dominant path, or one huge leak, obvious.",
  ],
  avoid: [
    "The flows go both ways between the same groups, or form loops. [Chord diagrams](/charts/chord-diagram) are built for that, and a Sankey with a cycle will not render.",
    "You have many tiny flows. A hundred ribbons under a pixel wide are drawn at least a pixel wide, so the picture claims more total flow than there is.",
    "You need exact values between every pair. A [sankey matrix](/charts/sankey-matrix) pairs the ribbons with a grid of squares that can be read exactly.",
    "The flow has a fixed number of named steps, such as a funnel. [Alluvial](/charts/alluvial) or [sankey funnel](/charts/sankey-funnel) are shaped for that.",
  ],
  read: [
    "Read left to right: sources on the left, sinks on the right. The columns are set by how far each node is from the start.",
    "Compare ribbon **widths**, not lengths or angles. Widths are on one scale across the entire chart.",
    "Check each node: what comes in is what goes out, except that a source has only outgoing flow and a sink only incoming.",
    "Hover a node to see its total and fade everything not connected to it. Hover a ribbon to see its source, target and value.",
  ],
  story: {
    title: "Where the grant went",
    paragraphs: [
      "Keiko was the finance lead of a small research institute called the Marlow Ocean Lab, and once a year she had to explain to a funding committee what had happened to their money. The committee had given 2.4 million, and in the past she had explained this with a table: 31 lines, three columns, one subtotal. The committee had a habit of reading the subtotal.",
      "This year's grant was different. It had been split between a field programme, a lab renovation and a data project, and the committee wanted to know how much of each had made it to research and how much had been absorbed by what the budget called \"overheads\", a word that has never once made a room feel better.",
      "Keiko's problem was that her table could say where each euro ended up but could not say where it had been. A euro that went to the field programme, then to ship charter, then to fuel, was three rows and no story. She needed the committee to follow the money the way the money had actually travelled.",
      "She exported the transactions as source, target and amount, with sources like Grant, Field, Lab and Data, and targets like Salaries, Ship charter, Equipment and Overheads. She passed them to `<Sankey>` in bui-charts with the defaults and nothing else. About ten minutes later she had a diagram.",
      "The grant node on the left split into three ribbons, and the field programme's ribbon was clearly the widest. From it, one fat band ran to ship charter, which then thinned into fuel and crew. The committee chair, who had not spoken in the previous meeting, traced a finger from the grant to a ribbon labelled Overheads and said it was thicker than the lab renovation. It was. It was 410,000.",
      "That one ribbon led to a forty-minute conversation, a review of the institute's charter contract and a decision to rebid it, which saved 120,000 in the next year. Keiko noted that the table had contained the same figure, on line 23, in a font size the committee had chosen not to read.",
      "She keeps the diagram as the first page of every annual report now. The committee asks fewer questions about subtotals and more about ribbons, which she considers progress.",
    ],
  },
  pitfalls: [
    "**Loops break the layout.** The graph must be acyclic. If \"A to B\" and \"B to A\" are both in the data, net them first or use a chord diagram.",
    "**Same names, same node.** A name is an identity. If \"Other\" appears at two different stages, it is a single node. Rename them if you meant two.",
    "**Many nodes, small canvas.** Columns are stacked with fixed padding, so a column with 30 nodes has very little height left per node and the ribbons compress to nothing.",
    "**Unbalanced nodes look odd.** A node that takes in 100 and sends out 80 is drawn at the larger size. The missing 20 is a real leak in the data, not a layout bug.",
    "**Ribbon colour is the source.** All the ribbons leaving a node share its colour. Colours do not mean anything beyond origin.",
  ],
  related: [
    { slug: "alluvial", why: "When items pass through several named stages and you want to follow their groups across them." },
    { slug: "sankey-matrix", why: "When you need exact values for each source and target pair." },
    { slug: "chord-diagram", why: "When flows run in both directions between the same groups." },
    { slug: "sankey-funnel", why: "When the stages are a funnel and you want to see what drops out at each step." },
  ],
  media: [
    {kind:"youtube",id:"wKIsxMRNodY",title:"What is a SANKEY diagram (+ examples) ??",channel:"Metabolism of Cities",note:"Explains Sankey diagrams with real flow examples."},
    {kind:"youtube",id:"d0DUVcXSo_w",title:"Sankey Diagrams",channel:"Sophos Education",note:"Short lesson on reading energy-flow Sankey diagrams."},
  ],
}
