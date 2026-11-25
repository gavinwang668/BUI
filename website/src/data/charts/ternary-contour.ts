import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A colour map and contour lines over every possible three-part mixture, for a property you can compute but not afford to measure everywhere.",
  intro: [
    "A scatter of samples on a [ternary diagram](https://en.wikipedia.org/wiki/Ternary_plot) answers the question \"which mixtures do I have?\" A contour answers the one that comes next: \"what would I get from the ones I do not have?\" If you have a model, a formula, or a fitted surface that turns a three-part mixture into a number (melting point, strength, viscosity, price), you can evaluate it everywhere and draw the result as a map.",
    "The idea comes straight from topographic maps. Colour says how high the value is, and contour lines connect points of equal value, so a tight crowd of lines means a steep slope and a wide gap means a plateau. The only difference is that the ground here is a triangle. That is why metallurgists and materials scientists love it: a phase diagram is basically a contour plot of a free energy that nobody could measure at every point, and nobody tried.",
    "The nice thing about handing the chart a function rather than data is that there is no noise, no gaps and no missing corners. The less nice thing is that the chart will happily draw whatever the function says, including nonsense. A formula that returns the wrong number at the corners still produces a handsome picture.",
  ],
  math: [
    {
      heading: "A function on a simplex",
      body: [
        "Your `field` is a function $f(a, b, c)$ of the three shares, which add to 1. The chart evaluates it on a regular lattice of compositions. With `resolution` $n$ (30 by default, clamped to 2 through 200), the lattice has $(n+1)(n+2)/2$ vertices at the points below, and the triangle is cut into $n^2$ small triangles.",
      ],
      tex: [
        "(a, b, c) = \\left(1 - \\tfrac{i + j}{n},\\; \\tfrac{j}{n},\\; \\tfrac{i}{n}\\right), \\qquad i, j \\ge 0,\\; i + j \\le n",
      ],
    },
    {
      heading: "Colour by cell",
      body: [
        "The values are scaled between the smallest and largest finite value $f_{\\min}$ and $f_{\\max}$ found on the lattice. Each small triangle is filled with the colour from the scheme (Viridis by default) at the mean of its three corner values, so the shading is a flat tile per cell, not a smooth gradient.",
      ],
      tex: [
        "t = \\operatorname{clamp}\\!\\left(\\frac{\\bar f - f_{\\min}}{f_{\\max} - f_{\\min}},\\, 0,\\, 1\\right), \\qquad \\bar f = \\tfrac{1}{3}(f_1 + f_2 + f_3)",
      ],
    },
    {
      heading: "Contours by marching triangles",
      body: [
        "For `levels` $L$ (8 by default), the contour values are evenly spaced strictly between the extremes, so the minimum and maximum themselves are never drawn. In each small triangle, an edge whose two ends lie on opposite sides of a level is crossed by the contour, and the crossing is placed by linear interpolation. A triangle with two crossings gets one line segment between them. Together the segments form the curves.",
      ],
      tex: [
        "\\ell_k = f_{\\min} + \\frac{k}{L + 1}\\,(f_{\\max} - f_{\\min}), \\quad k = 1, \\dots, L",
        "p = p_1 + \\frac{\\ell - f_1}{f_2 - f_1}\\,(p_2 - p_1)",
      ],
    },
  ],
  use: [
    "You have a **model or formula** over three parts: a fitted regression, a thermodynamic model, an empirical rule.",
    "You want to see where on the triangle a property peaks, drops or flips, and how fast it changes.",
    "You are designing a mixture and need to see the region that meets a spec, for instance strength above a limit and cost below one.",
    "You want a background for a ternary scatter, so measured samples sit on top of the model's prediction.",
  ],
  avoid: [
    "You only have a handful of measurements. Interpolating a surface from nine points is storytelling. Use the scatter and let the reader see the gaps.",
    "Your function is discontinuous or noisy. The picture will have jagged contours and tiles that look like glitches, because they are exactly what the function returned.",
    "You need the exact boundary of a phase region. A tile-based map is accurate to about one cell, which at the default is 1/30 of a side.",
    "You have more than three components, or a fourth that matters. Fix it at a value and make one triangle per value, or pick another chart.",
  ],
  read: [
    "Read the colour scale on the right: dark to bright means low to high for the default scheme, and the labels give the real values.",
    "Find the contour lines. Close together means steep. Far apart means flat. A closed loop is a peak or a pit, and the colour tells you which.",
    "Locate the corners. The value at a pure component is the colour at the corner, which is a useful check on whether the model makes sense.",
    "Hover any tile for its value, which is the mean of the tile's three corners.",
  ],
  story: {
    title: "The bulge in the 42 MPa line",
    paragraphs: [
      "Hamza formulated cement blends at a plant that made a million tonnes a year and had a recurring budget argument about limestone. The three ingredients that mattered for the grade he cared about were clinker, fly ash and limestone, and in theory every mix of them that added to 100% was legal. In practice the plant had been making about a dozen, because the ones next to those had worked.",
      "The sales team wanted a cheaper blend, which means less clinker. The quality team wanted the same strength at 28 days, which means not less clinker. Hamza sat between them, and every Monday had an email thread to prove it.",
      "A previous consultant had left him a regression: strength at 28 days as a function of the three shares, fitted on forty lab batches. It was not a perfect model, but it was a model, and the trouble was that nobody had ever looked at it as anything but a list of coefficients, which is the way people look at a will.",
      "He wrote the regression as a closure and passed it to a bui-charts `<TernaryContour>`, as `field`, with the corners named for the three ingredients and the viridis scheme left on. In about a second he had the whole triangle: a bright ridge of strength running diagonally from the clinker corner toward the middle, a dull valley near pure limestone, and a surprise.",
      "The 42 MPa contour, the one the plant needed, was not a smooth edge around a dozen blends. It bulged. At 60% clinker and 25% ash, it still sat above the line, where the plant had always put 70% to be safe. That was 10 points of clinker on a million tonnes.",
      "He did not trust it for a minute, which is the right way to trust a model. He ran six lab batches along the bulge. Five came in above 42 MPa, and the sixth, in a region the regression had marked as a cliff, fell off it exactly where the contours were packed tight.",
      "The plant moved to a 62% clinker blend three months later and saved about 6% of clinker cost on that grade. The sales team called it a victory. Quality called it a validated model. Hamza called it a Tuesday and took Wednesday off.",
    ],
  },
  pitfalls: [
    "**Trusting the extrapolation.** A fitted surface is only believable near the data. The chart draws it over the whole triangle, so mark or mask the regions your samples never visited.",
    "**Too few levels or too coarse a mesh.** Eight contours at resolution 30 is fine for a smooth property. A sharp ridge needs a higher `resolution`, up to 200 at a cost of CPU.",
    "**Constant fields.** If the function gives the same value everywhere, the range collapses and the contours disappear. That is the chart telling you something.",
    "**Closures that panic.** The field is called at every lattice point, including the corners, where two shares are zero. Make sure a log or a division in your function can cope.",
  ],
  related: [
    { slug: "ternary", why: "For plotting actual samples on the same triangle, which is what you want on top of a model." },
    { slug: "heatmap", why: "The rectangular cousin, for a property over two continuous variables." },
    { slug: "stuve", why: "Another chart with a physical grid behind it, for the atmosphere instead of a mixture." },
  ],
  media: [
    { kind: "youtube", id: "h8tFnC4pNGc", title: "Ternary Diagram Basics", channel: "Engineer Clearly", note: "Clear primer on reading a ternary diagram, the base for a ternary contour. Contours themselves are not covered." },
  ],
}
