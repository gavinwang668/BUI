import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A map with each region filled in a colour that stands for its value.",
  intro: [
    "Put a number on every region and the obvious thing to do is colour the regions. The obvious thing has been done since at least 1826, when the French engineer [Charles Dupin](https://en.wikipedia.org/wiki/Charles_Dupin) shaded departments of France by how many people in each could read. The name arrived later, from the Greek for place and multitude, and the format has stayed popular ever since because everyone can read a map before they can read anything else.",
    "That popularity is both the good news and the bad. A [choropleth](https://en.wikipedia.org/wiki/Choropleth_map) answers \"where?\" better than any chart in this library. It also answers \"how much?\" worse than almost all of them, because colour is a poor ruler and area is a worse one. Big, empty regions dominate the picture, small, dense ones disappear, and a map of raw counts will turn out to be a map of where people live, which is something you could have guessed without a chart.",
    "So use it for what it does well: showing that a value changes across space, and where the high and the low parts lie. Use rates and not counts, pick a scale a human can read, and treat the picture as a way to ask a question, not as the answer. A good choropleth makes people point at a region and say \"why there?\" and that is the most useful thing a map can do.",
  ],
  math: [
    {
      heading: "Joining values to shapes",
      body: [
        "You pass two datasets: the regions, as named polygons, and the values, one row per region. The chart joins them by name. A region with a matching row takes its value, and a region without one is drawn in neutral grey with the tooltip \"No data\". If two rows share a name, the later one wins. Values that are not finite are treated as missing too.",
      ],
    },
    {
      heading: "Value to colour",
      body: [
        "The colour is the scheme sampled at a normalised position. By default the range is the minimum and maximum of the values that matched a region. You can fix it with `range`, which is what you need for comparing two maps or centring a diverging scheme on zero. Values outside a fixed range are clamped to its ends. The default scheme is Blues.",
      ],
      tex: [
        "t_i = \\operatorname{clamp}\\!\\left(\\frac{v_i - v_{\\mathrm{lo}}}{v_{\\mathrm{hi}} - v_{\\mathrm{lo}}},\\; 0,\\; 1\\right), \\qquad \\text{colour}_i = \\text{scheme}(t_i)",
      ],
    },
    {
      heading: "Projection and fitting",
      body: [
        "A region is a list of rings of longitude and latitude, and every ring is filled, so islands work but holes are not cut out. The projection turns each point into plane coordinates before drawing. Equirectangular uses longitude and latitude as they are. [Mercator](https://en.wikipedia.org/wiki/Mercator_projection) keeps longitude and stretches latitude, with the latitude clamped to 85 degrees so the poles do not shoot off the page. Plane takes $(x, y)$ as given, for tile maps, floor plans or schematic regions.",
      ],
      tex: [
        "\\text{Mercator: } y' = \\ln\\tan\\!\\left(\\frac{\\pi}{4} + \\frac{\\varphi}{2}\\right), \\quad \\varphi = \\operatorname{clamp}(\\text{lat}, -85^\\circ, 85^\\circ)",
      ],
    },
    {
      heading: "One scale for both axes",
      body: [
        "After projecting, the chart finds the bounding box of every ring, scales it with a single factor so it fits the area (leaving room on the right for the colour bar), and centres it. One factor for both axes means shapes keep their proportions. Hover a region for its name and value, formatted to `decimals` if you set it.",
      ],
      tex: [
        "k = \\min\\!\\left(\\frac{W}{x_{\\max} - x_{\\min}},\\; \\frac{H}{y_{\\max} - y_{\\min}}\\right)",
      ],
    },
  ],
  use: [
    "You have **one value per region** and the question is *where*: unemployment by county, incidence by district, votes by constituency.",
    "The regions are meaningful to your audience, such as countries, states or postcodes, and the values are normalised (rates, percentages, per-capita figures).",
    "You want to spot spatial patterns: clusters, gradients, a hot spot that spills across a border.",
    "You need a non-geographic map: a floor plan, a seating chart or a hex-tile map of states, using the `Plane` projection.",
  ],
  avoid: [
    "The values are **raw counts**. Big regions will have more of almost anything, and the map will show population. Divide by something first.",
    "Region sizes are very uneven. A giant empty region will shout and a tiny city-state will whisper. Consider a tile map or a bubble layer.",
    "You need to compare exact values. Colour allows about five to seven distinguishable levels. A sorted bar chart is better.",
    "The data is a point process (accidents, shops). Colouring regions by their count hides where in the region the points are.",
  ],
  read: [
    "Look at the colour bar first and decide what the dark end means. Is high good or bad?",
    "Scan for patterns across neighbouring regions: a gradient, a cluster, a sharp edge at a border.",
    "Check the grey regions. They are missing, not zero, and a missing region in the middle of a pattern is usually a data issue.",
    "Hover a region for its name and value, and remember that the size of the region on the map is not the size of the population.",
  ],
  story: {
    title: "The map of where people live",
    paragraphs: [
      "Prisha was a data analyst at a regional health authority, and her first big assignment came with a fine title and a bad brief. The director wanted a map of flu hospitalisations, by district, for a briefing on Friday. Of her thirty-one districts, one was a city of two million and several were farms with a nice view.",
      "She made the map the way anyone would. She counted admissions per district, coloured them with the blues scheme and showed it to a colleague, who looked at it and said, \"So it's the city.\" She said it was not only the city. He said, kindly, that it was only the city.",
      "He was right. The darkest district was the city with 1,420 admissions, and the next-darkest were the three biggest towns, in order of size. The map was an accurate portrait of the census, and it had told the director nothing he did not know.",
      "She divided by the district's population and multiplied by 100,000, then passed the new numbers to a bui-charts `<Choropleth>`, with the regions loaded once into a signal and joined to the rates by district name. The scheme stayed blue, with a fixed `range` so that next week's map would use the same colours.",
      "The city turned out to be about average. The dark spots moved to a pair of rural districts in the north-east, with rates nearly three times the regional figure, and a pale patch in the middle that on inspection turned out to be grey: two districts had not reported at all. Those, as it happens, were the ones that shared a border with the dark ones.",
      "The Friday briefing changed. The director asked for a mobile vaccination team for the north-east instead of more beds in the city, and for a phone call to the two quiet districts. The call took ten minutes. The vaccination team went out the following Tuesday and the north-east's rate fell by a third within a month.",
      "The first map had the right data and the wrong question. The second had the right question, and a single division.",
    ],
  },
  pitfalls: [
    "**Counts instead of rates.** Normalise by population, area or whatever makes regions comparable. The map should show something other than where people live.",
    "**Name mismatches.** The join is by exact name. \"St. Louis\" and \"Saint Louis\" are different regions, and the unmatched one is grey. Check the grey ones.",
    "**Colour range tied to the data.** The default range is the min and max, so one outlier washes out everything else. Set `range` to clip, and keep it fixed when comparing maps.",
    "**Wrong projection.** Mercator inflates high latitudes, so northern regions look bigger and more important than they are. For an overview of the world, prefer the default equirectangular, and for anything schematic, `Plane`.",
    "**Rebuilding the regions every frame.** Keep the regions in a signal, because the map is only recomputed when they or the data change.",
  ],
  related: [
    { slug: "heatmap", why: "When the two dimensions are not space, such as day against hour." },
    { slug: "mekko", why: "When you want to show both the size of each group and its share, rather than one colour." },
    { slug: "waffle", why: "When you want counts of units to read at a glance, with no geography." },
  ],
  media: [
    { kind: "youtube", id: "nRqdTBKwYeU", title: "Normalizing and Classifying Choropleth Maps", channel: "ArcGIS", note: "Explains why raw counts mislead and how normalising and classification change what a choropleth says." },
    { kind: "youtube", id: "FCTKT0KaQdc", title: "Choropleth Mapping: MAUP, normalization, dasymetric mapping", channel: "Rhumbline Studio", note: "Covers the modifiable areal unit problem, the other big way choropleths mislead." },
  ],
}
