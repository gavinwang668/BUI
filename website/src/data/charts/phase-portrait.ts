import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "The arrows a differential equation draws on the plane, plus the paths that follow them.",
  intro: [
    "A time plot of a two-variable system answers the question *what does it do from here*. A phase portrait answers the harder question: *what does it do from everywhere*. Instead of plotting x and y against time, you plot y against x and let time vanish into the shape of the path. Starting points become curves, and the curves together tell you where the system settles, where it circles, and where it runs away.",
    "[Henri Poincaré](https://en.wikipedia.org/wiki/Henri_Poincar%C3%A9) built much of this way of thinking in the 1880s because the equations he wanted, for the motion of planets, had no tidy formulas. He could not write the answer down, so he drew the flow. That is still the right instinct. Most interesting systems have no closed-form solution, and a picture of the flow is often the only honest summary.",
    "The chart gives you two layers. The faint arrows are the direction field: at each grid point, which way does the system want to go? The bold lines are trajectories you ask for by giving starting points, integrated forward until they run out of time or fall off the page.",
  ],
  math: [
    {
      heading: "The system",
      body: [
        "You supply a function `field` that maps a point $(x, y)$ to a velocity $(\\dot x, \\dot y)$. That is the autonomous system below: time does not appear on the right-hand side, which is why a trajectory is a curve in the plane rather than a surface in space.",
      ],
      tex: [
        "\\dot x = f(x, y), \\qquad \\dot y = g(x, y)",
      ],
    },
    {
      heading: "Trajectories by fourth-order Runge-Kutta",
      body: [
        "Each start in `starts` is integrated with the classical [RK4 scheme](https://en.wikipedia.org/wiki/Runge%E2%80%93Kutta_methods) and a fixed step $h$ (`step`, 0.01 by default) for $\\lceil T/h \\rceil$ steps, where $T$ is the `duration` (10 by default). With $\\mathbf z = (x, y)$ and $F = (f, g)$:",
      ],
      tex: [
        "k_1 = F(\\mathbf z_n),\\quad k_2 = F\\!\\left(\\mathbf z_n + \\tfrac{h}{2}k_1\\right),\\quad k_3 = F\\!\\left(\\mathbf z_n + \\tfrac{h}{2}k_2\\right),\\quad k_4 = F(\\mathbf z_n + h\\,k_3)",
        "\\mathbf z_{n+1} = \\mathbf z_n + \\frac{h}{6}\\left(k_1 + 2k_2 + 2k_3 + k_4\\right)",
      ],
    },
    {
      heading: "When a path stops",
      body: [
        "A trajectory ends early if a coordinate stops being finite or leaves a box that is the plotted window padded by one full width and height on every side. That keeps a blow-up (a solution that heads to infinity in finite time) from stretching the axes or burning the whole step budget. The axes themselves are fixed to `x_range` and `y_range`, so nothing outside the window is ever shown.",
      ],
    },
    {
      heading: "The direction field",
      body: [
        "The field is sampled on a regular grid of $(n+1)\\times(n+1)$ points, where $n$ is `arrows` (15 by default). At each point the velocity is normalised to unit length and scaled to a fixed cell size, so every arrow is the same length, with a head that is 35 percent of it. The arrows therefore show *direction only*; speed is not encoded. Points where the velocity is zero or not finite get no arrow, which is a handy way to spot equilibria: look for the holes.",
      ],
    },
  ],
  use: [
    "You have a two-variable system, such as predator and prey, a damped pendulum or a pair of coupled reactions, and want to know its long-term behaviour from any start.",
    "You are looking for equilibria, limit cycles or separatrices and want to see which starts lead where.",
    "You are teaching: a portrait shows a node, a saddle and a spiral in a way that a page of eigenvalues does not.",
    "You need to compare how a parameter change reshapes the flow, by drawing two portraits side by side.",
  ],
  avoid: [
    "The system has three or more variables. A plane can only show a slice or a projection, and trajectories that cross each other are no longer a contradiction.",
    "You want values at specific times. A phase portrait deliberately throws time away. Use a line chart of the solution.",
    "The system is [stiff](https://en.wikipedia.org/wiki/Stiff_equation). Fixed-step RK4 at 0.01 will be unstable or wildly wrong on equations with very different time scales.",
    "The system depends on time explicitly. Without autonomy the arrows are different at every moment, and one field cannot describe it.",
  ],
  read: [
    "Scan the arrows for patterns: do they converge on a point, circle one, or stream past each other?",
    "Look for gaps in the arrow grid. A missing arrow means zero velocity there, so that is a candidate equilibrium.",
    "Follow the bold trajectories. A path that spirals into a point means a stable focus. One that closes on itself is a cycle. One that peels away from a point means that point is unstable.",
    "Compare paths that begin close together. If they stay close, the region is stable, if they separate quickly, you are near a saddle.",
  ],
  story: {
    title: "Foxes, rabbits and a very patient ranger",
    paragraphs: [
      "Tomas was a wildlife ecologist for a regional park that had, over six years, lost its rabbits twice and its foxes once. The park board wanted to know whether this was incompetence, bad luck or arithmetic. Tomas suspected arithmetic, which is the least popular explanation at any budget meeting.",
      "The population counts went up and down in a pattern he had seen in textbooks: rabbits rise, foxes follow, rabbits crash, foxes starve. It looked cyclical but not regular. In a time series, the cycles had different heights and he could not tell whether the system was settling down, running away or just being its usual self.",
      "He needed to see all the possible histories at once. Rabbits on one axis, foxes on the other, and the rule that moves the pair from one state to the next drawn as a field the board could read like a weather map. He had a model with four parameters estimated from the park's counts, and that was enough to draw it.",
      "In bui-charts he used `<PhasePortrait>` with the model as the `field`, the plotted window set to a few hundred rabbits and a hundred foxes, and six starting points taken from the last six years. The arrows swirled anticlockwise around a point in the middle. The trajectories did not spiral in: they ran in loops around it, nested like the rings of a tree.",
      "That settled the argument. A system with closed loops has no tendency to calm down, and no particular reason to go extinct either, unless something kicks it. A small kick near the outer loop sent a population to within a handful of animals, which was exactly what had happened twice. The park's misfortune was not mismanagement but a bad place to start: both crashes began on the outermost loop.",
      "Tomas proposed a modest change, supplementary feeding in rabbit-poor years to pull the state back towards the middle. In the next four years the lowest rabbit count was 61, not 9, and no species disappeared. He presented the portrait to the board with one sentence, which was that the animals had been doing the maths all along.",
    ],
  },
  pitfalls: [
    "**Arrow length lies about speed.** Every arrow has the same length. A region where the system moves a thousand times faster looks identical to a sluggish one, so do not read speed from the field.",
    "**Duration too short.** With the default of 10 time units, a slow system barely leaves its starting point. If trajectories look like stubs, raise `duration`.",
    "**Window too small.** Trajectories that leave a box one window wider on every side are cut. A clipped path may mean the system escapes, or just that your window is tight around a large cycle.",
    "**Impure fields.** The `field` closure is called many thousands of times. Keep it free of randomness and I/O, or the portrait will change between redraws.",
  ],
  related: [
    { slug: "quiver", why: "When the arrows come from measured vectors, such as wind or flow, not from an equation." },
    { slug: "poincare-map", why: "When you have a sequence rather than a flow and want to see what each value does to the next." },
    { slug: "bifurcation", why: "When you want to see how the long-run behaviour changes as a parameter is swept." },
  ],
  media: [
    {kind:"youtube",id:"UO_dgXa5szg",title:"Phase portraits of linear systems | Lecture 42 | Differential Equations for Engineers",channel:"Jeffrey Chasnov",note:"University lecture on reading phase portraits of linear systems: nodes, saddles and spirals."},
  ],
}
