import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Follow groups of items through several stages, one coloured band per origin.",
  intro: [
    "A before-and-after table is a polite way of hiding a story. The rows say that 412 people were in the free plan in January and 380 are in the paid plan in June, and you are left to work out who moved, who stayed, and who left through a door nobody marked. A two-column crosstab helps. Add a third stage and it stops being a table and starts being homework.",
    "An [alluvial diagram](https://en.wikipedia.org/wiki/Alluvial_diagram) is the answer to the homework. Each stage is a vertical column of boxes, one per category. Between neighbouring columns run bands whose width is the number of items that went from this box to that one. The name comes from the fan-shaped deposits a river leaves where it spreads out onto a plain. The version used for changing categories was popularised by [Rosvall and Bergstrom in 2010](https://doi.org/10.1371/journal.pone.0008694) for showing how clusters in a network change over time. It is a close relative of the [Sankey](/charts/sankey), and the family resemblance is deliberate.",
    "What separates them is the data you start with. A Sankey wants links. An alluvial wants *items*: one row per person, ticket or passenger, with a category at every stage. The chart counts the links for you, and keeps each band in the colour of the category it started in, so you can follow a group across the whole picture without a magnifying glass.",
  ],
  math: [
    {
      heading: "Rows become paths",
      body: [
        "Take $m$ rows and $S$ stages. Row $i$ has a label $c_{i,s}$ at each stage $s$ and a weight $w_i$, which is $1$ unless you give a value function. The chart makes one node for each distinct pair (stage, label), so the label \"Paid\" at stage 2 and the label \"Paid\" at stage 3 are different boxes, as they should be.",
        "Every row contributes a link between each pair of neighbouring stages. Links are keyed by three things: the node they leave, the node they enter, and the label the item had at **stage 1**. Rows that share all three are merged and their weights add up. That third key is what produces the colouring: a band belongs to the group an item started in, however many stages it has since passed through.",
      ],
      tex: [
        "L(a, b, o) = \\sum_{i \\,:\\, (c_{i,s}, c_{i,s+1}, c_{i,1}) = (a, b, o)} w_i",
      ],
    },
    {
      heading: "From counts to ribbons",
      body: [
        "Links go to the same layout engine as `<Sankey>`, with nodes in each stage justified to fill the height. A node's height is proportional to the larger of what flows into it and what flows out, so when the total is conserved from stage to stage the boxes in every column add up to the same height. Nodes have a default width of $12$ pixels and $12$ pixels of padding between them.",
        "Each ribbon runs from the right edge of one node to the left edge of the next along a smooth S-curve: a smoothstep that eases the vertical position from the start height $y_0$ to the end height $y_1$. A ribbon is never thinner than one pixel, so even a link of three items out of ten thousand stays hoverable.",
      ],
      tex: [
        "y(t) = y_0 + (y_1 - y_0)\\,\\bigl(3t^2 - 2t^3\\bigr), \\qquad t \\in [0, 1]",
      ],
    },
    {
      heading: "What is not conserved",
      body: [
        "If an item has an empty or missing label at some stage, it still gets a node (with that label, even if the label is the empty string), so check your data first. Rows with a zero or negative weight add nothing. And because nodes are matched by name within a stage, a typo (\"Pro\" and \"pro\") makes two boxes. The chart will not guess.",
      ],
    },
  ],
  use: [
    "You have one row per item and a category at **each of several stages**: class to outcome, plan to plan, school to degree to job.",
    "The question is where groups end up *and* where they came from, and you want one picture that answers both without a legend lookup.",
    "Stage-to-stage changes are the story. A flat bar chart per stage tells you the totals. The alluvial tells you who swapped places.",
    "You have two to five stages and a handful of categories in each. That is the range where bands stay distinguishable.",
  ],
  avoid: [
    "You already have a link table with amounts and nodes that can connect anywhere. Use [Sankey](/charts/sankey) and skip the conversion.",
    "There are more than about eight categories per stage. Bands thin out into noodles and the crossings turn the middle into a lace curtain.",
    "The stages are not really a sequence. If the columns are unrelated questions in a survey, the ribbons imply a flow that does not exist.",
    "You need the exact counts. Hover gives them, but if the reader will want a number from each band, add a table or use [Sankey Matrix](/charts/sankey-matrix).",
  ],
  read: [
    "Read left to right. Each column is a stage. The box height is how many items are in that category at that stage.",
    "Pick a colour at the left and follow it. Because bands keep their origin colour, a colour that ends up in a different box at the end is a group that changed category along the way.",
    "Look for fat bands that cross others: those are the big movers. Thin bands that stay level are the stayers.",
    "Hover a box to highlight every band that touches it, or a band for its count. Everything else fades to a third of its opacity.",
  ],
  story: {
    title: "Three classes, one very quiet staircase",
    paragraphs: [
      "Priyanka taught the introductory statistics course at a mid-sized university, which is the course people take so they can later say they were never good at statistics. Her department had a theory, repeated at every meeting with the confidence of an unverified fact, that students who got a C in the first course were basically a lost cause for the second.",
      "She did not think it was true, but she had no way to show it. She had grades in a spreadsheet: 1,840 students, a grade in the intro course, a grade in the second, a grade in the capstone. Three columns of letters. A table of three-way counts has 125 cells and no personality.",
      "The question was about paths, not totals. Where did the C students *go*, and did they go anywhere different from the B students? That is exactly what a stack of category columns with coloured bands is built to show, and exactly what a pile of crosstabs is built to hide.",
      "She loaded the rows into `<Alluvial>` from bui-charts with three `<Stage>` children, one per course, and left the value function out so each student counted once. The A band in the first column was green, the B band blue, the C band amber, and she watched the amber go somewhere.",
      "It went, to her private delight, mostly into B and C in the second course, and a thick amber band curved into an A in the capstone. A little over a quarter of the C students finished with an A or a B. The department theory was not dead, exactly, but it looked like it needed a walking stick.",
      "She printed it on the one printer that worked and taped it to the door of the chair's office. The chair read it, said nothing, and the following semester the \"C means lost\" slide quietly vanished from the orientation deck. Priyanka counts that as a peer-reviewed result.",
      "A table would have told them the same thing. It would have taken four meetings.",
    ],
  },
  pitfalls: [
    "**Labels are matched exactly.** \"Paid\", \"paid\" and \"Paid \" make three boxes in the same column. Clean your strings first.",
    "**Colour means *origin*, not current state.** A band keeps the colour of its first-stage label all the way across. That is the point, but it surprises people who expect colour to match the box it enters.",
    "**Stage order is the order of the children.** The chart does not reorder stages. Put them in the order you want read, left to right.",
    "**Item weights add.** If your rows are already aggregated, return the count from `value`. If they are not, leave it out, and every row is one.",
    "**Too many categories.** Beyond about eight per stage, group the tail into \"Other\" before you pass the data in.",
  ],
  related: [
    { slug: "sankey", why: "When you already have source, target and amount, and the nodes can be anywhere in the flow." },
    { slug: "sankey-funnel", why: "When the flow is a funnel fed by several channels, with the leak at each step shown." },
    { slug: "sankey-matrix", why: "When you need exact values between two sets, not a visual trend." },
  ],
  media: [
    { kind: "youtube", id: "6BYac2Pmnno", title: "How to make Alluvial Diagram", channel: "RAWgraphs", note: "Walkthrough of building an alluvial diagram from tabular data with the open-source RAWGraphs tool." },
    { kind: "youtube", id: "usNQkqxp6ow", title: "A quick guide to Sankey Diagrams", channel: "Flourish", note: "About the closely related Sankey diagram. Explains how to read flows between stages." },
  ],
}
