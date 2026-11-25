import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "How strongly an antenna sends or hears in each direction, drawn as a polar plot in decibels.",
  intro: [
    "An antenna is not a lamp. A lamp sends light roughly everywhere and you are happy with that. A good antenna sends its energy where you want it and as little as possible elsewhere, and the picture that says how well it manages is the [radiation pattern](https://en.wikipedia.org/wiki/Radiation_pattern): stand at the antenna, face every direction in turn, and mark how much signal there is.",
    "The shape is the product. A broad round blob is an omnidirectional antenna, good for a phone, useless for a link to a hilltop. A narrow main lobe with a ring of small side lobes is a dish. A lobe that points forward with almost nothing behind it is a directional antenna you can aim. Each of these shows up instantly in polar form, and almost none of it shows up in a table of numbers.",
    "The plot is almost always in [decibels](https://en.wikipedia.org/wiki/Decibel), because the range is vast: the main lobe may be ten billion times stronger than the sidelobes, and on a linear scale they would not exist. The chart puts your gain figures on a decibel scale with the strongest direction at the rim and the floor at the centre, so every lobe, down to the ones you wish were not there, gets a place on the page.",
  ],
  math: [
    {
      heading: "From gain to radius",
      body: [
        "Each sample is an angle $\\theta$ in degrees, clockwise from the top, and a gain $g$ in dB for each pattern you add as a child. A sample lands at radius $r$ and angle $\\theta$ with the rule below, where $g_{\\max}$ is the dB at the rim (`max`) and $g_{\\min}$ the dB at the centre (`min`), and $R$ is the plot's radius in pixels. Gains below the floor are clamped to the centre, and gains above the rim to the rim.",
      ],
      tex: [
        "r = R\\,\\operatorname{clamp}\\!\\left(\\frac{g - g_{\\min}}{g_{\\max} - g_{\\min}},\\; 0,\\; 1\\right)",
      ],
    },
    {
      heading: "Defaults, rings and spokes",
      body: [
        "If you give no `max`, it is the highest finite gain in any of the patterns, rounded up to a multiple of 10 dB; if there is none, it is 0. The floor defaults to $g_{\\max} - 40$ dB. So a pattern normalised to 0 dB at its peak spans 0 to minus 40 dB, and the radial scale is linear in decibels: each ring is an equal step in dB.",
        "`levels` rings (4 by default) are drawn at equal dB steps and labelled in dB, and twelve spokes mark every 30 degrees. A pattern that covers the full circle is closed and lightly filled (fill opacity 0.1). An open pattern covering a partial sweep is drawn as a line only, so a one-sided measurement does not claim data it does not have.",
      ],
    },
    {
      heading: "Relationship to the physics",
      body: [
        "The chart does not compute a pattern; it plots what you measured or simulated. For orientation, a uniform linear array of $N$ isotropic elements with spacing $d$ and wavelength $\\lambda$ has the array factor below, and the gain in dB is $20\\log_{10}|AF|$ of the field, which is $10\\log_{10}$ of the power. The half-power beamwidth is where the pattern has fallen 3 dB from its peak, which is the figure most data sheets quote.",
      ],
      tex: [
        "AF(\\theta) = \\frac{\\sin(N\\psi/2)}{N\\sin(\\psi/2)}, \\qquad \\psi = \\frac{2\\pi d}{\\lambda}\\cos\\theta",
        "g(\\theta)\\,[\\text{dB}] = 10\\log_{10}\\frac{P(\\theta)}{P_{\\max}}",
      ],
    },
  ],
  use: [
    "You are comparing an antenna design against its specification: main lobe direction, beamwidth, sidelobe level, front-to-back ratio.",
    "You have two cuts of the same antenna, such as the E-plane and the H-plane, and want them drawn on the same axes.",
    "You are reviewing measurements from an anechoic chamber, or a simulation, and want to see where the unwanted lobes are.",
    "You want a picture that an RF engineer will read without a caption.",
  ],
  avoid: [
    "You want the full three-dimensional pattern. A polar chart is one slice. You can draw several, but not a sphere.",
    "Your data are linear power or voltage. Convert to dB first, because the radial scale is linear in dB and will crush the small lobes.",
    "You want to compare values exactly. Rings are labelled in steps. A table or a plain [XY Chart](/charts/xy-chart) of gain against angle is better for reading numbers.",
    "You are plotting something that is not directional. Categories arranged round a circle belong in a [Spider](/charts/spider) or a [Nightingale Rose](/charts/nightingale-rose), not a pattern.",
  ],
  read: [
    "Find the main lobe: the longest petal, pointing in the direction of peak gain. Check that it points where you intended.",
    "Measure its width at the ring 3 dB below the peak. That is the half-power beamwidth.",
    "Look at the smaller petals. The tallest side lobe sets your sidelobe level, and the lobe opposite the main one is the back lobe, the front-to-back ratio being the gap between them.",
    "Hover to read the gain of every pattern at the nearest sample angle. Hover snaps to samples, so a coarse measurement will snap coarsely.",
  ],
  story: {
    title: "The dish that heard the neighbours",
    paragraphs: [
      "Ruth worked for a rural internet provider that ran radio links between hilltops, and she had a complaint: a customer in a valley complained that the link dropped every evening around seven. The link margin was fine. The weather was fine. The only thing that happened at seven was that, as far as anyone could tell, somebody three kilometres away turned on a microwave link of their own.",
      "The antenna was a small panel with a stated beamwidth of 20 degrees and a stated sidelobe level of minus 25 dB. Those are the numbers on the box. They tell you the antenna passes its test, not what it does at 140 degrees, where it turns out the interference was coming from.",
      "She wanted the whole picture, from every direction, for the actual antenna as installed, with the measured pattern of the panel on top of the manufacturer's. She had a rotator and a signal generator and an afternoon in a field, which is the complete equipment list for most antenna measurements and a good reason to enjoy them.",
      "She recorded the gain every five degrees and passed both sets to `<RadiationPattern>` as two `<TrendSeries>` children, with the floor at minus 40 dB. The manufacturer's pattern was a tidy lobe with small, even petals. Her measurement had the same main lobe, and then, around 140 degrees, a bump that rose to minus 14 dB: a side lobe 11 dB higher than the specification.",
      "A plot like that is a small argument by itself. The neighbour's link sat at 142 degrees from her boresight. The bump was listening to the neighbour's transmitter, which raised its power in the evening peak. The data sheet pattern was a drawing of a good antenna. Hers was a picture of a specific one, with a bracket bent by two degrees and a mounting pole that reflected into the back half.",
      "She straightened the bracket, moved the dish 40 centimetres to take the pole out of the way, and re-measured. The lobe at 140 degrees dropped to minus 26 dB, inside the specification. The link has not dropped at seven since, and the neighbour has never been told that they were, briefly, the most powerful thing within three kilometres.",
      "Ruth has since asked for the measured pattern on every installation. The manufacturer's drawing, she says, is a promise, and the plot is a receipt.",
    ],
  },
  pitfalls: [
    "**Angle convention.** Angles are in degrees, clockwise from the top. A pattern stored counter-clockwise from the right will look rotated and mirrored. Convert it before plotting.",
    "**Linear data in dB clothing.** If the values are linear power, the whole lobe looks like a thin spike and the sidelobes vanish. Convert with $10\\log_{10}$ first.",
    "**Min and max chosen badly.** The defaults assume a peak near 0 dB and a 40 dB floor. A broad pattern of 8 dBi gain with a floor of minus 32 dBi has no detail near the rim. Set `min` and `max` explicitly.",
    "**Comparing plots with different scales.** Two patterns drawn with different floors look equally sharp but are not. Fix `min` and `max` for every plot in a comparison.",
  ],
  related: [
    { slug: "spider", why: "When the axes are categories and not angles in space, and you want a radar-style comparison." },
    { slug: "nightingale-rose", why: "When you want polar bars for quantities that repeat around a circle, like months or compass directions." },
    { slug: "smith-chart", why: "When the question is how well the antenna is matched to its feed across frequency, not which way it points." },
  ],
  media: [
    {kind:"youtube",id:"rwciKjvHFfY",title:"Inside Wireless: Radiation Diagram",channel:"RF elements",note:"Explains how to read antenna radiation diagrams: lobes, beamwidth and nulls."},
    {kind:"youtube",id:"gJZPX4OMn94",title:"Radiation Patterns of Antenna (Isotropic Antenna, Directional Antenna & Omnidirectional Antenna)",channel:"Engineering Funda",note:"Lecture-style comparison of isotropic, directional and omnidirectional patterns."},
  ],
}
