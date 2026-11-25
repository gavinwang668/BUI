import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A conversion funnel fed by several channels, with the leak at every step drawn as its own band.",
  intro: [
    "Every product team owns a funnel, and every funnel owns a sad little trapezoid. Visitors, sign-ups, activations, purchases: each bar is shorter than the last, and the only thing it tells you is that people are lost. Which people? Where did they come from? Did the paid ads lose them at the first step, or did they sail through and give up at checkout? The trapezoid has no comment.",
    "A Sankey funnel keeps the funnel idea and adds the two things a trapezoid leaves out. On the left, the *channels* people entered through: organic search, email, a partner, whatever you track. Across the middle, the steps, with every channel's flow shrinking as it goes. And at every step, a separate band peeling off downward into a **Dropped** box, so the loss is a shape you can look at instead of a difference you have to subtract.",
    "The result is a fusion of two charts that have been around for a very long time: the funnel from sales textbooks and the flow diagram from [Captain Sankey](https://en.wikipedia.org/wiki/Sankey_diagram)'s 1898 steam-engine paper, in which he showed where the energy of an engine went. The engine in this case is your sign-up page, and the heat is mostly leaving through the password field.",
  ],
  math: [
    {
      heading: "What goes in",
      body: [
        "You give the chart one row per channel and a function $n(c, k)$ that returns how many items from channel $c$ reached stage $k$, for $k = 0, \\dots, K-1$. Stage $0$ is the entry count: how many came in at all. The chart does not trust your funnel to be well behaved. It enforces that a stage can never have more than the one before it, and never fewer than zero.",
      ],
      tex: [
        "r(c, 0) = \\max\\bigl(0,\\; n(c, 0)\\bigr), \\qquad r(c, k) = \\max\\bigl(0,\\; \\min(n(c, k),\\; r(c, k-1))\\bigr)",
      ],
    },
    {
      heading: "Bands and drops",
      body: [
        "Every channel gets a node in the first column. For each stage $k$ there is a node in column $k + 1$, shared by all channels. A band of width $r(c, k)$ runs from the previous node of channel $c$ (the channel node for $k = 0$, otherwise stage $k - 1$) to stage $k$, in the colour of the channel.",
        "From the second stage on, the items that did not make it are drawn as a second band, from the previous node into a **Dropped after** sink in the same column. Its width is the loss. One sink exists per stage, so the drops of all channels pile up into a single box that tells you how many left at that step, and the bands feeding it keep their channel colours.",
      ],
      tex: [
        "\\ell(c, k) = r(c, k-1) - r(c, k), \\qquad k \\ge 1",
        "\\text{height of stage } k \\;\\propto\\; \\sum_c r(c, k)",
      ],
    },
    {
      heading: "Layout",
      body: [
        "Nodes are placed by the same layout engine as `<Sankey>` with left alignment, so a node's column is fixed by its stage and its height is proportional to the larger of its inflow and its outflow. The last stage has no outflow, so its height is just what arrives. The default node width is $12$ pixels and the default node padding $12$ pixels. Ribbons are never drawn thinner than one pixel.",
        "Setting `drops` to false skips the sinks entirely and you get only the shrinking bands, which is a cleaner picture when the drops are not the story. Hovering a node shows it and every band touching it, and hovering a band shows its value.",
      ],
    },
  ],
  use: [
    "You have a **multi-step funnel with several entry channels** and the questions are *which channel leaks where* and *how big is each leak*.",
    "Stakeholders keep asking \"where do we lose them?\" and a table of conversion percentages has not ended the conversation.",
    "You want to compare channels without drawing one funnel per channel and asking the reader to align them in their head.",
    "The count falls from step to step and the shape of that fall is part of the story: a cliff at step two reads very differently from a gentle slope.",
  ],
  avoid: [
    "There is only one channel. Then the dropped bands are the only extra information and a plain [funnel](/charts/funnel) says it more simply.",
    "The funnel has loops, re-entry or optional paths. The chart assumes a one-way ordered sequence of steps.",
    "Your counts are not nested. If step three can contain people who never reached step two, the chart will quietly clamp them to the previous stage and understate the number.",
    "You have more than about twelve channels. The first column becomes a stack of slivers. Group the tail first.",
  ],
  read: [
    "Start on the left and read the channel column: box heights are entry counts, so the tallest channel is where most of the traffic comes from.",
    "Follow a channel's colour across. The band narrows after each step, and the narrowing is exactly the drop.",
    "Look at the **Dropped after** boxes. A tall one means that step is where the most people leave. A thin one means the step is not your problem.",
    "Compare colours inside a Dropped box: if one channel's colour dominates it, that channel has a specific problem at that step.",
    "Hover a stage or band for its amount. Everything unrelated fades back.",
  ],
  story: {
    title: "Forty-one percent of the problem was a single button",
    paragraphs: [
      "Tomasz ran growth for a small app that turned receipts into spreadsheets. The company had three ways in: a search page that ranked embarrassingly well, a newsletter written by the founder, and a partnership with an accounting forum whose moderators had strong opinions about commas. The funnel had four steps: landing, sign-up, first receipt uploaded, subscription.",
      "Every Monday the team looked at the funnel, and every Monday it looked like a funnel. 28,000 people landed, 9,100 signed up, 3,300 uploaded a receipt, 610 paid. The conversation was always the same: everyone agreed the numbers should be bigger, and nobody agreed which of them.",
      "His task was to stop the Monday argument, which meant he needed to show not only how many people were lost but which channel they were lost from, and at which step, in one picture that a founder could understand in the time it takes to unmute a call.",
      "He gave the per-channel counts to `<SankeyFunnel>` in bui-charts, one row per channel with an array of four counts, and named the stages in order. The chart did the subtraction he had been doing in a notebook for a month.",
      "The result was impolite. The big dark band from the search channel went through sign-up like a train and then fell off a cliff at the first receipt. The Dropped-after-sign-up box was almost entirely that one colour: 4,900 people who had made an account and never uploaded anything. The newsletter band, tiny but loyal, barely leaked at all.",
      "He did not need a regression. He went and tried the upload flow as a stranger would, found that the \"Add receipt\" button sat below the fold on a laptop, and moved it. Three weeks later the search channel's loss at that step had dropped from 71% to 52%, and paying customers went from 610 to 780 a month.",
      "The Monday meeting got shorter. The founder, to his lasting credit, asked why nobody had put the button there on day one, and everyone looked at the window.",
    ],
  },
  pitfalls: [
    "**Counts are clamped.** A stage cannot exceed the stage before it, and negative values become zero. If your data is not nested, the chart will silently shrink the later stage. Fix the data, not the chart.",
    "**The first value is the entry count.** `value(item, 0)` is how many entered, not how many reached step one after some filter. Make the first stage the true start.",
    "**One Dropped box per stage, not per channel.** The channels are told apart by colour inside it. If you want separate sinks, build separate charts.",
    "**Nothing is dropped before the first stage.** There is no band for people who were lost before stage zero, because the chart has no count for them.",
    "**Many near-zero channels clutter the first column.** Group them into \"Other\" before drawing.",
  ],
  related: [
    { slug: "funnel", why: "When there is a single sequence of steps and you only need the shrinking bars." },
    { slug: "sankey", why: "When flows are free-form and the nodes can connect in any left-to-right order." },
    { slug: "alluvial", why: "When you have one row per item and want to see how groups change category between stages." },
  ],
  media: [
    { kind: "youtube", id: "soiyjjXKJ8E", title: "SankeyEngine for HR Analytics - Recruitment Funnel with Sankey Diagrams", channel: "Excel Bootcamp", note: "A Sankey diagram used as a recruitment funnel, showing stage-to-stage drop-off." },
    { kind: "youtube", id: "G6dlvECRfcI", title: "How to read an LLNL energy flow chart (Sankey diagram)", channel: "Lawrence Livermore National Laboratory", note: "A clear walkthrough of how to read a Sankey diagram, using an energy flow chart rather than a funnel." },
  ],
}
