import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Every possible impedance squeezed inside one circle, so matching an antenna becomes a walk instead of an algebra problem.",
  intro: [
    "A radio engineer has a transmitter, a cable and an antenna, and the first law of the trade is that none of them agree. When the load on the end of a line does not match the line, part of the signal bounces back, power is wasted and amplifiers get warm in unhappy ways. The measure of this disagreement is a single complex number, the [reflection coefficient](https://en.wikipedia.org/wiki/Reflection_coefficient), and the question is always the same: how do I make it go to zero?",
    "Phillip Smith, an engineer at Bell Labs, answered that in 1939 with [a chart](https://en.wikipedia.org/wiki/Smith_chart) that maps the entire half-plane of passive impedances onto the inside of a unit circle. Infinite resistance, a number that would normally run off the page, becomes a point on the rim. A perfect match sits in the exact middle. The whole thing was drawn by hand, used with a ruler and a compass, and is still taught in universities, partly because it works and partly because nothing else is as pretty.",
    "A modern network analyser will happily print the numbers. People still ask for the Smith chart because the shape tells you what to *do*. Add a series inductor and your point slides along a circle. Add a shunt capacitor and it slides along another. Matching becomes a game of moving your dot to the centre, and you can see how far you have left to go.",
  ],
  math: [
    {
      heading: "From impedance to reflection coefficient",
      body: [
        "Each row gives a resistance $R$ and a reactance $X$ in ohms, measured against a reference impedance $Z_0$ (50 ohms unless you set `impedance`). Divide through by $Z_0$ to get the normalised impedance $z = r + jx$. The point plotted is the reflection coefficient $\\Gamma$, which is a function of $z$ and always lands inside the unit circle for a passive load.",
      ],
      tex: [
        "z = \\frac{R + jX}{Z_0} = r + jx, \\qquad \\Gamma = \\frac{z - 1}{z + 1}",
        "\\operatorname{Re}\\Gamma = \\frac{r^2 + x^2 - 1}{(r+1)^2 + x^2}, \\qquad \\operatorname{Im}\\Gamma = \\frac{2x}{(r+1)^2 + x^2}",
      ],
    },
    {
      heading: "The grid is two families of circles",
      body: [
        "The chart does not distort anything by hand. It draws the images of the lines $r = \\text{const}$ and $x = \\text{const}$ under the same map. Both turn out to be circles. A line of constant resistance $r$ becomes a circle centred on the horizontal axis; a line of constant reactance $x$ becomes an arc of a circle whose centre is above or below the axis at the right-hand edge, and only the part inside the unit disc is drawn.",
      ],
      tex: [
        "\\text{resistance circle: centre } \\left(\\tfrac{r}{1+r},\\, 0\\right),\\quad \\text{radius } \\tfrac{1}{1+r}",
        "\\text{reactance arc: centre } \\left(1,\\, \\tfrac{1}{x}\\right),\\quad \\text{radius } \\tfrac{1}{|x|}",
      ],
    },
    {
      heading: "Admittance and mismatch",
      body: [
        "The default grid values are $0.2, 0.5, 1, 2, 5$, and each is drawn for both signs of reactance. With `overlay` set to admittance, the same circles are reflected through the origin and drawn dashed; with both, you get the double grid used when you hop between series and shunt elements. A point's distance from the centre, $|\\Gamma|$, is the size of the reflection, and the [standing-wave ratio](https://en.wikipedia.org/wiki/Standing_wave_ratio) that most people actually quote follows from it.",
      ],
      tex: [
        "\\mathrm{VSWR} = \\frac{1 + |\\Gamma|}{1 - |\\Gamma|}",
      ],
    },
  ],
  use: [
    "You have **impedance versus frequency** from a network analyser or a simulator and want to see how well a load matches its line.",
    "You are designing a **matching network** and want to see how each added component moves the point.",
    "You want to compare several sweeps (antennas, filters, amplifier inputs) on a shared, familiar background.",
    "You are teaching transmission lines. The chart is a better textbook than any table.",
  ],
  avoid: [
    "Your audience does not do RF. Anyone else will see a circle of circles and quietly leave. Show them return loss in dB on a line chart.",
    "You need exact values. The chart is for the shape of the response, and a table or a magnitude-versus-frequency plot reads numbers better.",
    "Your load has *negative* resistance. The point leaves the unit circle, and the chart is not designed for it.",
    "You are plotting something that is not an impedance. Reusing the shape for decoration is allowed but will be noticed.",
  ],
  read: [
    "Find the centre. A point there is a perfect match, and the closer your trace gets to it, the less power bounces back.",
    "The horizontal axis is the pure-resistance line. Above it, the load is inductive. Below it, capacitive.",
    "Follow the trace in the order of the rows, which is normally frequency. A tight loop near the middle is a good wideband match. A trace that runs out to the rim is a load that reflects almost everything.",
    "Hover a point to read its reflection coefficient and magnitude.",
  ],
  story: {
    title: "The antenna that preferred the wrong band",
    paragraphs: [
      "Tomás built weather balloons in a garage that had lost its battle with a pile of cardboard, and the next release needed a 433 MHz telemetry link that could reach a ground station 40 kilometres away. The radio worked on the bench. The antenna worked on the bench. Together, in the field, they delivered about a third of the range he had promised his two investors, both of whom were his sisters.",
      "He tried everything a hobbyist tries. He moved the antenna. He moved the cable. He shortened the cable, then lengthened it, then shortened it again, and the range changed each time like a mood. A friend who does RF for a living looked at the setup and said, in the tone of a doctor looking at a cough, \"Have you measured the match?\"",
      "He had not. He borrowed a cheap vector analyser and swept the antenna from 400 to 460 MHz, which gave him 201 rows of resistance and reactance, and a spreadsheet that gave him a headache.",
      "He fed the columns into a bui-charts `<SmithChart>` with `impedance` left at 50 ohms. The trace did not go near the centre. It looped on the left half of the chart, at a point about 0.6 of the way to the rim at 433 MHz, and swung into the middle only around 470 MHz, a band he was not licensed for and was not using.",
      "The antenna was resonant, only at the wrong frequency. Cutting two millimetres from each element would have moved it, and he cut three, because garage engineering is an art. Ten minutes and a hacksaw later, the loop closed on the centre, and |Γ| at 433 MHz went from 0.6 to 0.08.",
      "The next flight carried the same radio, the same cable and the same cardboard, and held the link for 41 kilometres. His sisters asked what had changed. He said, truthfully, \"A point moved to the middle of a circle.\" They gave him a third round of funding on the strength of that sentence.",
      "A hacksaw is the cheapest tuner ever sold. The Smith chart is the only thing that tells you which way to hold it.",
    ],
  },
  pitfalls: [
    "**Wrong reference impedance.** The chart is normalised by `impedance`. A 75 ohm system plotted at 50 ohms is a perfectly good chart of the wrong thing.",
    "**Plotting S11 as if it were Z.** If your instrument gives you Γ directly, convert back to $R$ and $X$ first. This chart takes impedance and does the map itself.",
    "**Order of rows.** The points are joined in the order you give them. Sort by frequency before plotting, or the line will zigzag over the grid.",
    "**Too many grid lines.** Adding more values to `grid` makes the chart look rigorous and unreadable. Five circles per family is a good number.",
  ],
  related: [
    { slug: "root-locus", why: "Another complex-plane chart from engineering, for the poles of a feedback loop instead of a load." },
    { slug: "radiation-pattern", why: "The other half of an antenna's story: where the energy goes once it leaves the line." },
    { slug: "eye-diagram", why: "When the question is signal quality on a high-speed link rather than power transfer." },
  ],
  media: [
    { kind: "youtube", id: "TsXd6GktlYQ", title: "#297: Basics of the Smith Chart - Intro, impedance, VSWR, transmission lines, matching", channel: "w2aew", note: "A bench engineer builds the Smith chart up from impedance and standing waves, and shows how it is used for matching." },
    { kind: "youtube", id: "rUDMo7hwihs", title: "Understanding the Smith Chart", channel: "Rohde & Schwarz", note: "A short manufacturer tutorial that connects the chart to what a network analyser draws." },
  ],
}
