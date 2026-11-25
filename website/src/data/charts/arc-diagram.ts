import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Nodes on a single line, with a half-circle arc for every link between them.",
  intro: [
    "Network diagrams have a reputation. You feed them forty nodes and a few hundred edges and get back a hairball: a glossy, force-directed tangle that looks impressive in a slide and answers no question at all. Everyone has seen one. Everyone has nodded at one. Nobody has ever read one.",
    "An [arc diagram](https://en.wikipedia.org/wiki/Arc_diagram) makes a deal with you. It takes away the freedom to put nodes anywhere and lays them out in a row, like beads on a string. Each link becomes a semicircle hung over the line between its two ends. Short arcs join neighbours, tall arcs join distant things, and because the order of the nodes is something *you* chose, the picture is only as tangled as your ordering is. Martin Wattenberg used the form in 2002 to show repeated structure inside strings, such as the repeats in a piece of music, and it has been quietly useful ever since for anything that has a natural order: chapters of a book, positions in a genome, steps in a process, names sorted by date.",
    "That is the whole trick. A force layout hides the order and shows you a shape. An arc diagram keeps the order and shows you the links. If your nodes have a sequence, the links *across* that sequence are usually the interesting part, and now they are the only part.",
  ],
  math: [
    {
      heading: "From link rows to a graph",
      body: [
        "Each row of your data is one link: a source name, a target name and, optionally, a value. Without a value function every link counts as $1$. Rows with a non-finite or non-positive value are dropped, and so are rows whose source and target are the same node, since a loop has no arc to draw.",
        "Nodes are created the first time their name appears, source before target, row by row. That first-appearance order *is* the horizontal order. The chart never sorts, clusters or optimises anything, so if you want a particular order, sort your rows first. Repeated pairs are not merged: two rows between the same nodes draw two arcs on top of each other.",
      ],
    },
    {
      heading: "Placing nodes and arcs",
      body: [
        "With $n$ nodes and a drawing width $W$, node $k$ (counting from $0$) sits on the baseline at the horizontal position below, which leaves a margin of $24$ pixels on both sides. A link between nodes $a$ and $b$ becomes the upper half of a circle whose diameter is the distance between them.",
        "If the widest arc would be taller than the room above the baseline, every arc is flattened by the same factor $s$, so arcs become half-ellipses but keep their relative heights. When everything fits, $s = 1$ and the arcs are true semicircles.",
      ],
      tex: [
        "x_k = 24 + \\frac{k}{n-1}\\,(W - 48), \\qquad r_{ab} = \\frac{|x_a - x_b|}{2}",
        "s = \\min\\!\\left(1,\\; \\frac{H_{\\text{free}}}{\\max_{ab} r_{ab}}\\right)",
      ],
    },
    {
      heading: "Weights and sizes",
      body: [
        "Stroke width is linear in the link value: with the largest value $v_{\\max}$, a link of value $v$ is drawn $1 + 7\\,v/v_{\\max}$ pixels wide, so the heaviest link is eight pixels and a feather-weight one is a hair above one. Each arc takes the colour of its **source** node.",
        "A node's dot is sized by its degree $d_k$, the sum of the values of every link that touches it, in either direction. The radius uses a square root so that the dot's *area* follows the total, which keeps one very busy node from swallowing the row.",
      ],
      tex: [
        "d_k = \\sum_{\\text{links at }k} v, \\qquad \\rho_k = 3 + 6\\sqrt{d_k / \\max_j d_j}",
      ],
    },
  ],
  use: [
    "Your nodes have a **natural order** and you want to see which ones are linked across it: chapters that reference each other, genome positions that interact, versions that depend on older versions.",
    "A force-directed layout of the same data would be a hairball, and you have already tried it. Moving the same graph onto a line costs you the freedom of position and pays you back in legibility.",
    "You want to compare *distances* between linked nodes. Short arcs versus tall arcs is a question the chart answers without being asked.",
    "The graph is moderately dense (tens of nodes, tens to a couple of hundred links) and you can sort it into an order that means something.",
  ],
  avoid: [
    "The order is arbitrary. If the nodes are first-seen-in-the-CSV order, tall arcs will look like findings and they are accidents of the file.",
    "You need to see direction. Every arc bows upward whichever way the link points, and the only cue is the colour of the source. Use [Sankey](/charts/sankey) or a [chord diagram](/charts/chord-diagram) when who-to-whom matters.",
    "You have hundreds of nodes. There is a limit to how many labels fit on a line, and the chart will start staggering them on two levels well before that.",
    "You want to find communities or hubs by structure alone. Without a good ordering, an arc diagram cannot cluster. A [node graph](/charts/node-graph) can.",
  ],
  read: [
    "Read along the baseline first: that is the order, and it carries the meaning. Left to right might be time, position, or whatever you sorted by.",
    "Look at arc height. A tall arc is a link between nodes far apart in the order. A run of tiny arcs is a neighbourhood talking to itself.",
    "Follow colour for direction. Each arc takes its source's colour, so a rainbow of arcs landing on one node means many different sources point at it.",
    "Judge importance by dot size and arc thickness: dots scale with total value touching the node, strokes with the value of that single link.",
    "Hover an arc for its source, target and value, or a dot for the node's total.",
  ],
  story: {
    title: "The footnotes that went the long way round",
    paragraphs: [
      "Ines edited a book on the history of tidal power, forty chapters, three authors, and a cheerful disregard on all sides for who had already said what. Her job in the final month was to make sure that the cross-references (\"as shown in Chapter 7\", \"see the discussion above\") pointed somewhere sensible and in the right direction. Forward references, in particular, make readers feel they have missed a meeting.",
      "She had every reference in a spreadsheet: 212 rows, each with a chapter that cites and a chapter that is cited. She drew them as a network, because that is what a network is for, and got a ball of string that looked like something a cat had finished.",
      "What she needed was not the shape of the web. It was a very specific question: which references jump far, and are the far ones forward or backward? That question only exists because chapters come in an order, and the force layout had thrown that order in the bin.",
      "She pasted the 212 rows into `<ArcDiagram>` from bui-charts, with `source` and `target` as the two chapter columns, and sorted the rows by chapter number first so the chapters appeared in reading order. No value function: every reference counted once.",
      "The chart looked like a bridge made by a committee. Most arcs were short little hops between neighbouring chapters, exactly as you would hope. But three enormous arcs soared from the last third of the book back to Chapter 3, all the same colour, all from Chapter 31. Chapter 3 was the one with the definitions, which Chapter 31 apparently needed but could not be bothered to restate.",
      "That was a finding, and an editorial one. Ines moved two definitions into a boxed glossary at the front, deleted nine of the long references and replaced them with a sentence each. The book lost six pages, and the index of cross-references went from 212 to 188, which the copy editor described as the first time in her career a diagram had reduced work.",
      "The tidal-power industry, to its credit, still does not read the book. The footnotes, however, now go where they say they go.",
    ],
  },
  pitfalls: [
    "**Order is the chart.** Nodes appear in first-seen order, source column first within each row. Sort your rows deliberately or you are drawing your data entry habits.",
    "**Self-links and non-positive values vanish.** A node that only links to itself will not appear, and a zero or negative value drops the whole link, not just its width.",
    "**Duplicate rows stack.** Two rows with the same pair draw two arcs in the same place. Sum them yourself if you want one thicker arc.",
    "**Direction is weak.** The arc always goes up, never has an arrowhead, and is coloured by source. If direction is the point, pick a chart that shows it.",
    "**Tall arcs get squashed.** When the widest arc would not fit, every arc is flattened equally. Heights stay comparable, but they are no longer semicircles.",
  ],
  related: [
    { slug: "chord-diagram", why: "Same links, but nodes sit on a circle, so no order is implied and direction can be shown." },
    { slug: "sankey", why: "When the links carry amounts that flow one way and you want the widths to add up." },
    { slug: "node-graph", why: "When the layout should come from the structure of the links, not from an order you imposed." },
  ],
  media: [
    { kind: "youtube", id: "XOUJssmvz58", title: "Networks (Ch 9) I, Visualization Analysis & Design, 2021", channel: "Tamara Munzner", note: "University lecture on network visualization. Covers node-link and arc layouts, not specific to arc diagrams alone." },
    { kind: "youtube", id: "21ziL8j1iyc", title: "Create Stunning Arc Diagrams in Python | NetworkX & Matplotlib Tutorial", channel: "AI Evolutionary", note: "Hands-on tutorial that builds an arc diagram, using Python rather than this library." },
  ],
}
