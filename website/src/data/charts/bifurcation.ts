import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Where a repeated rule ends up, plotted for every value of its parameter.",
  intro: [
    "Take a very simple rule: multiply a number by a constant and by one minus itself, and feed the answer back in. Do that a few hundred times and ask where the number ends up. For a small constant it settles to a single value. Turn the constant up and it starts alternating between two values, then four, then eight, and then somewhere past 3.57 it stops repeating at all. Nothing about the rule changed except one dial.",
    "A bifurcation diagram shows all of those answers at once. The horizontal axis is the dial, the vertical axis is where the system lands after it has stopped fussing. It is the picture that made chaos theory legible to the public, and Robert May's 1976 paper on simple population models with very complicated dynamics is why biologists and mathematicians had to take it seriously. Mitchell Feigenbaum later found that the ratio between successive doublings approaches [the same number](https://en.wikipedia.org/wiki/Feigenbaum_constants), about 4.669, for a whole family of rules, whatever the details.",
    "That last fact is the reason people still draw it. A trivial equation does not just produce chaos, it produces the same chaos as a dripping tap, a turbulent fluid and a population of insects, and the diagram is the proof you can hold in one image.",
  ],
  math: [
    {
      heading: "Iterating a map",
      body: [
        "You pass a function `map` taking $(r, x)$ to the next state, and a `parameter` interval $[r_{\\min}, r_{\\max}]$. The chart evaluates $r$ at `steps` evenly spaced values (300 by default), and for each one runs the same procedure from the same seed `start` (0.5 by default).",
        "[The logistic map](https://en.wikipedia.org/wiki/Logistic_map) is the standard test case, and is just a function you give it:",
      ],
      tex: [
        "x_{n+1} = f(r, x_n) = r\\,x_n\\,(1 - x_n)",
      ],
    },
    {
      heading: "Throw away the transient, keep the attractor",
      body: [
        "For each $r$ the chart iterates `transient` times (300) without recording anything, to let the orbit forget where it started and fall onto the attractor. It then iterates `keep` more times (60) and records each value. If a value stops being finite, iteration for that $r$ stops and what was gathered so far is kept.",
        "The recorded values are sorted and near-duplicates are removed: two values are merged when they differ by at most $10^{-6}(1 + |x|)$. So a fixed point shows as one dot, a period-$p$ orbit as $p$ dots, and a chaotic band as up to 60 dots scattered across it. Dots are small (1.5 px) and drawn at 60 percent opacity, so regions the orbit visits often come out darker.",
      ],
    },
    {
      heading: "Cost, and what the picture can hide",
      body: [
        "The total work is `steps` $\\times$ (`transient` + `keep`) calls to your `map`: about 108,000 with the defaults, and it is recomputed only when a numeric prop changes. A periodic orbit with period larger than `keep` shows only part of itself, and a periodic window narrower than one step $\\Delta r = (r_{\\max}-r_{\\min})/(\\text{steps}-1)$ can be missed entirely. The y axis is unfixed and fits the data.",
      ],
      tex: [
        "\\Delta r = \\frac{r_{\\max} - r_{\\min}}{\\text{steps} - 1}, \\qquad \\delta = \\lim_{k\\to\\infty}\\frac{r_k - r_{k-1}}{r_{k+1} - r_k} \\approx 4.669",
      ],
    },
  ],
  use: [
    "You have a discrete-time rule with a tunable parameter, such as a growth rate, a feedback gain or a drive amplitude, and want to see how its long-run behaviour changes.",
    "You are hunting for the parameter value at which behaviour changes qualitatively: onset of oscillation, period doubling, the edge of chaos.",
    "You are teaching nonlinear dynamics and want one image that shows order, doubling and chaos.",
    "You want to find windows of order inside a chaotic region, the periodic stripes in the fog.",
  ],
  avoid: [
    "Your system is continuous in time. Sample it first with a Poincaré section, or use a [Phase Portrait](/charts/phase-portrait) of a single parameter choice.",
    "You have several state variables. The chart plots one. Pick a variable you can defend, or a projection.",
    "You want a quantity such as a Lyapunov exponent, which this chart does not compute. It draws the attractor, not its sensitivity.",
    "Your map is expensive to evaluate. A hundred thousand calls is nothing for a quadratic and a lot for a simulation.",
  ],
  read: [
    "Start at the left. A single thin line is a stable fixed point: the system always goes to the same value.",
    "Follow it until it splits. Each fork is a period-doubling: the system now alternates between two values, then four.",
    "Find where the lines dissolve into a smear. That is chaos, and the density of the dots shows where the orbit spends its time.",
    "Look inside the smear for clean stripes, such as the wide window near $r = 3.83$ in the logistic map. Order returns, briefly, before breaking down again.",
  ],
  story: {
    title: "Fourteen thousand fish and a bad year",
    paragraphs: [
      "Nomvula ran a small fisheries lab for a river authority, and her problem was a fish. The authority stocked a trout called the silver chub (a fictional name for a real kind of headache) and set the annual catch limit on a simple model of how a generation turns into the next. Every few years the population crashed for no reason anyone could name, and a crash cost the authority about a fifth of its licence income.",
      "The model had one parameter that mattered, the growth rate. It had been estimated at 2.8 and the board had been told, correctly, that at 2.8 the population settles to a steady number. A second estimate from a new survey put the rate at 3.3. The board asked whether that was the same thing but slightly better. It was not.",
      "What Nomvula needed was a picture of behaviour against growth rate, for the whole plausible range, so that the board could see not one answer but the shape of all of them. She could have described it as a period-doubling cascade, which would have lost the room by the second hyphen.",
      "She used `<Bifurcation>` with the model as `map`, a parameter range from 2.5 to 4, 600 steps and a longer transient, since the population took a while to settle. One thin line ran along the left, split at 3 into two, split again at about 3.45, and then went to fog. A vertical rule drawn by hand at 2.8 sat on the thin line. A second, at 3.3, sat in the part with two branches.",
      "That was the whole conversation. At 3.3 the model does not settle on a steady number, it alternates between a high year and a low year, forever. The crashes were not random, they were the low branch of a two-year cycle, and the authority had been setting the same catch in good years and bad. Nobody had been negligent, they had been averaging.",
      "The new rule set a catch limit that tracked the cycle, about 30 percent lower in the low year and 12 percent higher in the high one. Over six years the low-year population stayed above 9,000 fish, up from a trough of 4,100, and licence income stopped bouncing.",
      "Nomvula's slide had one line of text on it: the fish had never been unpredictable, just two-faced.",
    ],
  },
  pitfalls: [
    "**Too short a transient.** If orbits near a bifurcation take a long time to settle, the plot shows the journey rather than the destination. Raise `transient` and see whether the picture changes.",
    "**Too few steps.** Thin periodic windows are narrower than the gap between sampled parameters. If a narrow stripe seems to flicker in and out, increase `steps`.",
    "**Map signature.** The closure receives the parameter first and the state second: `map(r, x)`. Swapping them gives a plausible and entirely wrong plot.",
    "**Escaping orbits.** If the map sends values off to infinity for some parameters, those columns come out empty or sparse. For the logistic map, keep $r \\le 4$ and the start in $[0, 1]$.",
  ],
  related: [
    { slug: "poincare-map", why: "When you want to see the rule behind one particular parameter value, as a return map of its sequence." },
    { slug: "phase-portrait", why: "When you want to see the flow of a continuous system, not the long-run values of a discrete one." },
    { slug: "heatmap", why: "When you want to show density across two dimensions and do not have a rule, only measurements." },
  ],
  media: [
    {
      kind: "youtube",
      id: "ovJcsL7vyrk",
      title: "This equation will change how you see the world (the logistic map)",
      channel: "Veritasium",
      note: "Builds the logistic map and its bifurcation diagram from scratch, including the period doubling, the chaotic region and the Feigenbaum ratio.",
    },
  ],
}
