import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Stars placed by temperature and brightness, where their whole life story turns out to be a map.",
  intro: [
    "Scatter a few thousand stars by how hot they are and how bright they are, and they do not fall in a cloud. They fall in bands: a long diagonal stripe, a puddle of faint hot things in the lower left, a scattering of enormous cool ones in the upper right. Each band is a stage of a star's life, which means a chart with no time axis has somehow drawn the history of the universe's furnaces.",
    "[Ejnar Hertzsprung](https://en.wikipedia.org/wiki/Ejnar_Hertzsprung) made the first version around 1911 and [Henry Norris Russell](https://en.wikipedia.org/wiki/Henry_Norris_Russell) arrived at the same picture independently a couple of years later, which is why it carries both their names and a hyphen that nobody has ever had the energy to dispute. The convention they left behind is the strange part: temperature runs from hot on the left to cool on the right, backwards from every other chart you own. It is a historical accident, inherited from spectral classes, and the diagram has been defended against reform ever since.",
    "The chart keeps the convention. You hand it a temperature and a luminosity per star, and it flips the axis, takes the logarithm of both and colours each dot by temperature using a blackbody gradient from deep orange to blue-white. Optionally it also sizes the dot by radius, which turns out to be the quickest way to see why a red giant is called one.",
  ],
  math: [
    {
      heading: "Two logarithmic axes, one of them reversed",
      body: [
        "Each row gives an effective temperature $T$ in kelvin and a luminosity $L$ in solar units. Both axes use a base-10 logarithmic scale. The temperature axis is reversed, so the hottest stars are on the left, and the luminosity axis runs upward. Stellar temperatures run from a couple of thousand kelvin to tens of thousands, and luminosities from a millionth of the Sun's to a million times it, so linear axes would crush almost everything into one corner.",
      ],
    },
    {
      heading: "Why radius draws straight lines",
      body: [
        "A star's luminosity is set by its surface area and its temperature. For a blackbody with the [Stefan-Boltzmann](https://en.wikipedia.org/wiki/Stefan%E2%80%93Boltzmann_law) constant $\\sigma$, and measured relative to the Sun, the two are tied by the relations below. Taking logarithms turns the power laws into a straight line of fixed slope.",
      ],
      tex: [
        "L = 4\\pi R^{2}\\sigma T^{4} \\quad\\Longrightarrow\\quad \\frac{L}{L_\\odot} = \\left(\\frac{R}{R_\\odot}\\right)^{2}\\left(\\frac{T}{T_\\odot}\\right)^{4}",
        "\\log_{10} L = 2\\log_{10} R + 4\\log_{10} T + \\text{const}",
      ],
    },
    {
      heading: "Colour and size",
      body: [
        "Colour is a function of $\\log_{10} T$ only, mapped on a gradient of eight stops from orange-red through white to pale blue. It is a decoration of the axis, not an independent measurement, and it is there because it is how the stars actually look. If you pass a `radius` accessor, each dot's size is mapped from the radius column onto a range of 3 to 30 px; without it all dots use `point_size` (6 px by default). Dots have no outline.",
        "A line of constant radius is a line of constant $\\log R$, so on these axes it has a fixed slope of 4 in $\\log L$ per unit of $\\log T$. Bigger stars sit higher and to the right, which is why the giants and supergiants live in the upper right and the white dwarfs, at roughly the size of the Earth, live at the bottom.",
      ],
    },
  ],
  use: [
    "You have a catalogue with temperature and luminosity (or absolute magnitude converted to luminosity) and want to classify stars by evolutionary stage.",
    "You are plotting a star cluster and want to find the turn-off point from the main sequence, which dates the cluster.",
    "You want to show students where the Sun is, what it will become and what happens to the stars that are too big to stay.",
    "You want a size channel for radius, to see that a bright cool star is simply a very large one.",
  ],
  avoid: [
    "You have only colour and apparent magnitude. Without distances, apparent brightness mixes distance with luminosity and the bands blur into a cloud.",
    "Your temperatures come from a rough colour index with large errors. The chart will make precise claims about stars that you cannot make.",
    "You need to compare stellar masses. Mass is not a plotted variable here. It is read from the position on the main sequence, and only there.",
    "The audience will read the left-to-right axis as time or growth. Explain the reversal once, aloud.",
  ],
  read: [
    "Find the main sequence, the diagonal band from the hot bright upper left to the cool faint lower right. Most stars, the Sun among them, are on it, burning hydrogen.",
    "Look above it and to the right: red giants and supergiants. They are cool, so they would be dim, but they are so large that they are luminous.",
    "Look below it and to the left: white dwarfs. Hot, faint, tiny. They are what is left when a star runs out.",
    "Hover a dot for its values. If you passed a radius, compare dot sizes: along a diagonal of equal slope, size should grow steadily.",
  ],
  story: {
    title: "The cluster that was older than it looked",
    paragraphs: [
      "Zainab was an astronomy PhD student with a catalogue of 4,800 stars in a globular cluster and a supervisor who asked, every Monday, whether she had the age yet. The data were fine. The temperatures came from spectra and the luminosities from distance-corrected photometry. She had a spreadsheet, a deadline, and a supervisor who treated the word soon as a unit.",
      "A cluster's age is hidden in a single place on a diagram: the point where stars leave the main sequence. Massive stars burn fast and leave first, so the older the cluster, the lower the turn-off. But that only works if you plot it the traditional way, and the first thing she had tried was a plain scatter with linear axes, which looked like a comet stuck to the corner of the page.",
      "What she needed was the standard picture, logarithmic on both sides, hot on the left, with colour so she could trust her eye in the crowd. A paper figure would be done in a plotting package. She wanted a first look inside the tool she was already using to clean the data, with the cluster in a table she could filter.",
      "She passed the catalogue to `<HrDiagram>` in bui-charts with temperature and luminosity columns, and the radius from her fitting as the optional size. The main sequence appeared as a clean orange-to-white stripe, and above it, a thick branch of enlarged dots curved up and to the right. The turn-off was easy to see: a sharp knee at about 6,300 K and 1.5 solar luminosities.",
      "That knee corresponds to stars of about 0.85 solar masses, and from the lifetimes of such stars her age came out at roughly 11 billion years, close to her first guess but with something new: a small cluster of bright blue stars above the knee that had no business being there. They turned out to be stars that had merged, [blue stragglers](https://en.wikipedia.org/wiki/Blue_straggler), and the cluster was dense enough to cook a few.",
      "Zainab gave the age on Monday with an error bar and a footnote. Her supervisor looked at the figure, asked where the radius legend was, and then, for the first time in her experience, said nothing about the schedule.",
      "The paper's second figure was the same chart with the stragglers circled. The cluster had been hiding a small crime scene in plain sight, and it took logarithms and a reversed axis to show it.",
    ],
  },
  pitfalls: [
    "**Reading the x axis as normal.** Temperature decreases to the right. Fix this in your head before you annotate the chart.",
    "**Non-positive values.** Both axes are logarithmic, so a temperature or luminosity of zero or below cannot be placed. Filter those rows out first.",
    "**Luminosity units.** The axis is labelled in solar luminosities. If you pass watts or magnitudes, the labels will be wrong by many orders of magnitude without any warning.",
    "**Colour is not an extra variable.** It repeats the temperature. Do not tell the audience that blue stars are special beyond being hot.",
  ],
  related: [
    { slug: "xy-chart", why: "When you want a general scatter with your own axes and scales rather than the astronomical convention." },
    { slug: "ternary", why: "When your three variables are proportions of a whole, as in mineral compositions, rather than two physical measurements." },
    { slug: "beeswarm", why: "When you want to compare the distribution of one variable, for example temperature, across groups of stars." },
  ],
  media: [
    {kind:"youtube",id:"_EtlJCfaxdc",title:"Stars and Galaxies: The Hertzsprung-Russell Diagram",channel:"Conceptual Academy",note:"Clear classroom explanation of the main sequence, giants and white dwarfs on the H-R diagram."},
  ],
}
