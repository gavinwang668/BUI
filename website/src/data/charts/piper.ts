import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Six ions, three plots and one diamond: the fingerprint of a water sample, and a map of what it has mixed with.",
  intro: [
    "Groundwater is a thin solution of six main ions, and every aquifer has its own recipe. Calcium, magnesium, sodium and potassium on the positive side. Bicarbonate, sulfate and chloride on the negative. A hydrogeologist who wants to know where a spring has been, or whether two wells are drinking from the same rock, starts by asking how those six compare, and every comparison needs a table that nobody wants to read.",
    "In 1944 Arthur Piper of the US Geological Survey published [a graphic method](https://en.wikipedia.org/wiki/Piper_diagram) that solves this. It uses two triangles, one for the cations and one for the anions, and a diamond between them that combines both. Each water sample becomes three dots: a point in each triangle, and a point in the diamond that sums up its overall type. The same sample is in three places at once, which is rude for people but great for chemistry.",
    "The result is a plot where families of water show up as clusters, and the process that links them shows up as straight lines. Mix a fresh spring with seawater and the samples line up between the two end points, in all three panels. Dissolve some gypsum into the mix and the line bends. It is one of the few water-quality charts where the geometry is also the chemistry.",
  ],
  math: [
    {
      heading: "Concentrations as shares of charge",
      body: [
        "Each sample needs six concentrations in milliequivalents per litre, because charge balance is what makes the triangles meaningful. If you have mg/L, divide by the [equivalent weight](https://en.wikipedia.org/wiki/Equivalent_weight) of the ion. Write the cations as calcium $\\mathrm{Ca}$, magnesium $\\mathrm{Mg}$ and sodium plus potassium $\\mathrm{Na}$, and the anions as bicarbonate $\\mathrm{HCO_3}$, sulfate $\\mathrm{SO_4}$ and chloride $\\mathrm{Cl}$. Each group is normalised on its own, to 100%.",
      ],
      tex: [
        "P = \\mathrm{Ca} + \\mathrm{Mg} + \\mathrm{Na}, \\qquad N = \\mathrm{HCO_3} + \\mathrm{SO_4} + \\mathrm{Cl}",
        "\\text{cation shares: } \\tfrac{\\mathrm{Ca}}{P},\\, \\tfrac{\\mathrm{Mg}}{P},\\, \\tfrac{\\mathrm{Na}}{P} \\qquad \\text{anion shares: } \\tfrac{\\mathrm{HCO_3}}{N},\\, \\tfrac{\\mathrm{SO_4}}{N},\\, \\tfrac{\\mathrm{Cl}}{N}",
      ],
    },
    {
      heading: "Two triangles",
      body: [
        "Both triangles are drawn side by side with a small gap, and use the same barycentric placement as the [ternary](/charts/ternary) chart. In the cation triangle calcium is the bottom left, sodium plus potassium the bottom right and magnesium the top. In the anion triangle bicarbonate is the bottom left, chloride the bottom right and sulfate the top. A sample with no positive or no negative charge cannot be normalised, so it is skipped.",
      ],
    },
    {
      heading: "The diamond",
      body: [
        "The diamond sits in the gap, above both, and has two axes. One is the share of sodium plus potassium among the cations. The other is the share of sulfate plus chloride among the anions. Together they give a point $(u, v)$ in the unit square, and the diamond is that square turned 45 degrees, so that the four corners are the four classic water types: bottom, calcium bicarbonate. Top, sodium chloride. Left, calcium sulfate or chloride. Right, sodium bicarbonate.",
      ],
      tex: [
        "u = \\frac{\\mathrm{Na}}{P}, \\quad v = \\frac{\\mathrm{SO_4} + \\mathrm{Cl}}{N}, \\qquad x = x_0 + \\tfrac{s}{2}(u - v), \\quad y = y_0 - s\\,\\tfrac{\\sqrt{3}}{2}(u + v)",
      ],
    },
  ],
  use: [
    "You have **a set of water analyses** and want to group them by type, source or aquifer at a glance.",
    "You want to check for **mixing** between two end members: samples on a mixing line fall on a straight line in each panel.",
    "You want to see a change over time or along a flow path, such as a well that moves from calcium bicarbonate towards sodium chloride.",
    "Your audience is hydrogeologists, geochemists or water managers, who already read the diamond.",
  ],
  avoid: [
    "Concentrations matter. A Piper diagram throws the totals away, so a pristine spring and a brine can land on the same dot. Show total dissolved solids elsewhere.",
    "You have minor ions, nutrients or metals to compare. The six in the chart are all it knows.",
    "You have only one or two samples. A bar chart of the six values is clearer.",
    "Your values are in mg/L and you have not converted them. The chart will happily show a wrong chemistry.",
  ],
  read: [
    "Start in the two triangles. Where is the cloud: near Ca, Mg or Na plus K for the cations, and near bicarbonate, chloride or sulfate for the anions?",
    "Move to the diamond. The corner nearest the point gives the water type: bottom for calcium bicarbonate, top for sodium chloride, left for calcium sulfate or chloride, right for sodium bicarbonate.",
    "Look for straight lines through the same samples in all three panels. That is mixing, and the end points are your source waters.",
    "Hover any point for the six shares. Colours are groups, such as aquifers or seasons, and clicking the legend hides one.",
  ],
  story: {
    title: "Two wells, one argument",
    paragraphs: [
      "Rutendo was the hydrogeologist for a mid-sized town in a dry region, which meant her working hours were divided between the town's water and the town's opinions about it. The new municipal well, drilled in March, tasted salty. The older well, a kilometre east, did not. The mayor wanted to know whether the new one was drawing from the same aquifer and, if not, whether it was drawing from something he should be worried about.",
      "She had forty-one samples from the two wells and from eleven monitoring boreholes, with the six main ions in mg/L. The first thing she did was divide each by its equivalent weight, because charge balance is the one thing a Piper diagram insists on. The second was to open a spreadsheet and realise that nobody would ever read it.",
      "She loaded the samples into a bui-charts `<Piper>` with the aquifer as `group`. The old well and its boreholes clustered in the bottom of the diamond, calcium bicarbonate, as healthy as a glass of rain. The new well did not join them. It sat near the top corner, sodium chloride, with a cloud of boreholes scattered between the two.",
      "The cloud was a line. In all three panels the points ran straight from the fresh cluster to the salty well, which is what mixing looks like. Nothing special was dissolving. Two waters were being drawn into the same pump in different proportions.",
      "That moved the question from chemistry to hydrogeology. Where was the salty end member? The chloride share of the end point matched a deep brine already mapped about 120 metres below the new screen, and the pump was pulling it up. The mixing line even told her the proportion: about a fifth of what came out of the tap was brine.",
      "The fix was a bit of unglamorous plumbing: a shallower screen, a lower pumping rate, and a blending valve while the town waited for both. Chloride at the tap dropped from 310 mg/L to 85 within two weeks. The mayor asked for a one-page summary, and Rutendo gave him the diamond, with the line drawn on it in pen.",
      "Chemistry is a long story told in numbers. The Piper diagram is the version that fits in a mayor's pocket.",
    ],
  },
  pitfalls: [
    "**mg/L instead of meq/L.** The chart assumes charge-equivalent units. Convert with the equivalent weights (Ca 20.04, Mg 12.15, Na 22.99, K 39.10, HCO3 61.02, SO4 48.03, Cl 35.45) before plotting.",
    "**Forgetting to merge ions.** Pass sodium plus potassium as one value, and bicarbonate plus carbonate as one value, or the shares will not add up the way the chart expects.",
    "**Bad analyses.** If cations and anions are very different in meq/L, the lab data is off. The diagram will not tell you, since each side is normalised on its own. Check the charge balance first.",
    "**Reading too much into the diamond.** It is a projection of both triangles, so two different waters can share a point. Use the triangles to tell them apart.",
  ],
  related: [
    { slug: "ternary", why: "The building block: each Piper triangle is a ternary diagram for three ions." },
    { slug: "stuve", why: "Another Earth-science chart where the geometry carries the physics, for the atmosphere." },
    { slug: "choropleth", why: "When you want to see where the water types are on a map." },
  ],
  media: [
    { kind: "youtube", id: "Zi10G92biY4", title: "How to Interpret Piper Trilinear Diagram Hydrochemical Characteristics of Groundwater Samples", channel: "Geojay GIS Solution", note: "Shows how to read the two triangles and the diamond to name a water type." },
  ],
}
