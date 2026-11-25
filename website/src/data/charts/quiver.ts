import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A grid of little arrows showing which way a field points and how hard it pushes at each spot.",
  intro: [
    "Wind, water, heat, electric force: a surprising amount of the physical world is a [vector at every point](https://en.wikipedia.org/wiki/Vector_field) in space. A heatmap can tell you how strong the wind is. It cannot tell you it is blowing straight at your house. For that you need direction, and the oldest trick for showing direction is the one a child would pick: draw an arrow.",
    "A quiver plot (the name comes from the arrows, the same way a quiver holds them) puts one arrow at each sample point. The tail sits at $(x, y)$, and the arrow runs along $(u, v)$, so its direction is the direction of the field and its length is the strength. Plot enough of them and the arrows line up into streams, swirls and sinks, the way iron filings line up around a magnet without anyone drawing a single curve.",
    "The catch is that an arrow chart is a dialogue between you and the scale. Make the arrows too long and they pile on each other like commuters on a platform. Make them too short and the picture looks like a field of tiny commas. Most of the work in a good quiver plot is deciding how long one unit of wind should be, which is why this chart picks a sensible answer for you and lets you override it.",
  ],
  math: [
    {
      heading: "From a vector field to a line segment",
      body: [
        "Each row gives a position $(x_i, y_i)$ and a vector $(u_i, v_i)$. Rows where any of the four values is not finite are dropped before anything else. The arrow for row $i$ is a line from the tail to the tip, with a scale factor $k$ converting units of the vector into units of the position axes.",
      ],
      tex: [
        "\\text{tail}_i = (x_i,\\; y_i), \\qquad \\text{tip}_i = (x_i + k\\,u_i,\\; y_i + k\\,v_i)",
        "m_i = \\sqrt{u_i^2 + v_i^2}",
      ],
    },
    {
      heading: "How the default scale is chosen",
      body: [
        "Unless you pass `scale`, the factor $k$ is chosen so the longest arrow just about fits in the space each arrow owns. Let $R_x$ and $R_y$ be the extents of the tails along each axis, $n$ the number of arrows and $m_{\\max}$ the largest magnitude. The typical spacing is the side of the square each arrow would get if the tails were spread evenly, and the longest arrow is drawn at 90% of that.",
      ],
      tex: [
        "s = \\sqrt{\\frac{R_x\\,R_y}{n}}, \\qquad k = \\frac{0.9\\,s}{m_{\\max}}",
      ],
    },
    {
      heading: "Arrowheads and the aspect ratio",
      body: [
        "Each head is drawn at the tip with a size of $0.3\\,k\\,m_i$, but never more than $0.3\\,s$, so short arrows keep proportionate heads and long ones do not grow harpoons. If every vector is zero, or all the tails sit on one spot so that $s = 0$, the factor falls back to 1.",
        "With `equal_aspect` on (the default) one unit is the same number of pixels along $x$ and along $y$. That matters here more than for most charts: if the axes were stretched differently, an arrow pointing at 45 degrees in the data would point somewhere else on the screen, and the chart would be lying about direction.",
      ],
    },
  ],
  use: [
    "You have a **vector at each point**: wind or current samples, gradients of a function, force fields, flow measured by particle tracking.",
    "You want to see the *shape* of the field (rotation, convergence, a stagnation point) and not just its strength.",
    "Your samples sit on a rough grid, or at least spread fairly evenly, so the arrows do not overlap.",
    "You are explaining a model: plotting the gradient of a loss surface or the drift of a differential equation is the fastest way to show where things roll.",
  ],
  avoid: [
    "You only care about magnitude. A heatmap or a contour plot is calmer and easier to read than a thicket of arrows.",
    "You have thousands of irregular points. Arrows collide, and the chart becomes a haystack of needles. Resample onto a grid first.",
    "Your vectors span many orders of magnitude. The longest arrow sets the scale, so everything else shrinks to a dot. Plot the direction only, or take a log of the magnitude.",
    "The two components have different units (price and volume, say). The angle of the arrow then means nothing.",
  ],
  read: [
    "Find the longest arrows first: they mark the strongest part of the field, and by default they have been scaled to just touch their neighbours.",
    "Follow the arrows head to tail. They trace the streamlines even though no streamline is drawn.",
    "Look for the places where arrows swing around a point (a vortex), point away from it (a source) or toward it (a sink), and where they shrink to nothing (a stagnation point).",
    "Hover the tail of an arrow to read where it starts and its magnitude.",
  ],
  story: {
    title: "The pond that wouldn't drain",
    paragraphs: [
      "Mirela worked as a hydrologist for a small municipality in the Danube plain, which meant her job was telling people where water went and, more often, why it had not gone there. The problem that spring was a retention pond behind a new housing estate. It was designed to drain in two days. Three weeks after the first rain it still looked like a very sincere attempt at a lake.",
      "The contractor insisted the outlet pipe was fine. The residents insisted the pond was a swamp. The council insisted on a report by Friday. Mirela had a field team, a handheld flow meter and a spreadsheet with 240 readings of water speed in two directions, taken on a loose grid across the pond. She also had a heatmap of speed that showed, with great confidence, that water was moving. It did not say where to.",
      "Speed was never the issue. Direction was. A pond that drains has water leaning, collectively, towards the outlet. A pond that does not drain has water leaning in some other direction, politely, for reasons of its own.",
      "She loaded the spreadsheet into a bui-charts `<Quiver>`, with the two position columns as `x` and `y` and the two velocity columns as `u` and `v`. The default scale did the rest. The arrows lined up into a slow clockwise swirl in the north half of the pond, then curled into a tight little eddy a few metres from the outlet, where a corner of the concrete apron turned out to stick out like a shoulder.",
      "The eddy was recirculating the water the outlet was supposed to be removing. The longest arrow in the chart was about 4 cm/s. The arrows at the outlet itself were smaller than a grain of rice, which in a quiver plot is as close as the water gets to a confession.",
      "The contractor shaved 60 cm off the apron and put in a short guide wall. Mirela repeated the survey a month later with the same 240 points. The swirl had gone, every arrow within five metres of the outlet pointed at it, and the pond emptied in 38 hours. The report was filed on Thursday, a day early, which the council treated as a mild act of aggression.",
      "A number says how much. An arrow says where to. Pond owners need both, but only one of them can be blamed on a wall.",
    ],
  },
  pitfalls: [
    "**Arrows that do not fit.** If you hand-set `scale` too high, arrows from neighbouring points overlap and form a scribble. Start with the default and change it by a factor of two at a time.",
    "**One huge outlier.** The longest arrow sets the default scale, so a single bad reading squashes everyone else. Clip or inspect outliers before plotting.",
    "**Stretching the axes.** Turning `equal_aspect` off lets you fill a wide panel, but angles are no longer true. Do it only when x and y are the same kind of quantity and you can live with the distortion.",
    "**Irregular sampling.** Dense clusters of tails look like strong fields, because arrows crowd and draw ink. The eye reads that as intensity. Sample evenly when you can.",
  ],
  related: [
    { slug: "phase-portrait", why: "When the vector field comes from a differential equation and you want the trajectories drawn through it." },
    { slug: "hodograph", why: "When you want the vector itself, at many heights or times, plotted as a single path." },
    { slug: "heatmap", why: "When only the strength of the field matters, not its direction." },
  ],
  media: [
    {kind:"youtube",id:"_Iub8f-RBJw",title:"Matplotlib Quiver Plot - Vector Field for an Electric Dipole",channel:"Dot Physics",note:"Builds a quiver plot of a vector field in Matplotlib. Same chart type, different tool."},
  ],
}
