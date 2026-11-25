import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Segments laid out end to end on a circle, with histogram tracks inside and ribbons linking positions on them.",
  intro: [
    "A genome is a very long string cut into 23 pieces, and the interesting questions are all about relationships between places on those pieces. Which region on chromosome 3 has been swapped with one on chromosome 12? Where are the mutations dense? A linear plot of one chromosome cannot show that, and 23 linear plots cannot show relationships between them.",
    "Circos, described by Martin Krzywinski and colleagues in a 2009 paper in Genome Research, solved it by putting all the chromosomes on one circle. Place each segment as an arc, with its length proportional to its size. Draw tracks of numbers inside it. Then connect positions with ribbons that cut through the middle. The picture is dense, and it is also the one genomics figure that people outside genomics recognise on sight.",
    "The layout has since left biology. Anything with long segments and positions along them fits: modules of a codebase, a year cut into months with events linked across it, or the 23 offices of a company that sometimes email each other. If your data is segments, tracks and links, this is the chart.",
  ],
  math: [
    {
      heading: "Segments become arcs",
      body: [
        "Each segment $s$ has a name and a length $L_s$ in whatever unit its positions are in (base pairs, days, lines of code). The lengths are laid end to end around the circle with a gap between segments (default 2 degrees, but never more than half an equal share). The angular scale is the same for every segment, so a segment twice as long takes twice the angle, and a position $x$ on segment $s$ becomes an angle linearly.",
        "Ticks, if `ticks` is set, are small marks outside the arc every so many units, counted from the start of each segment.",
      ],
      tex: [
        "u = \\frac{2\\pi - n\\,g}{\\sum_s L_s}, \\qquad \\theta(s, x) = \\theta_s + u\\,x, \\qquad \\theta_{s+1} = \\theta_s + u\\,L_s + g",
      ],
    },
    {
      heading: "Tracks: one histogram per ring",
      body: [
        "A track is a list of bins, each with a segment, a start, an end and a value. Tracks are stacked inward from the outer arc, one ring each, with a thickness of $0.38\\,R$ shared between them (capped at 34 px per track). In its ring, every bin becomes a wedge whose angular extent is its range and whose radial height is its value.",
        "Each track has its own scale. The tallest bin in a track fills the ring and the rest are in proportion. This is deliberate, because it lets you compare the shape of a track across segments, but it is also the main trap: two tracks of different units can both reach the top of their rings. Zero, negative and non-finite values are not drawn.",
      ],
      tex: ["h = \\frac{v}{\\max_{\\text{track}} v}\\,(\\text{ring thickness} - 2)"],
    },
    {
      heading: "Links: ribbons through the centre",
      body: [
        "A link joins a range on one segment to a range on another, $(s, [a_0, a_1])$ to $(t, [b_0, b_1])$, with a weight. It is drawn inside the innermost track. The two ends are arcs on that inner circle and the sides are curves bending toward the centre of the circle, which is what makes ribbons cut through the middle instead of hugging the edge.",
        "If both ends are narrower than about 0.004 radians, the link is drawn instead as a single thin curve whose stroke width grows from 0.8 px for the lightest link to about 3.8 px for the heaviest. Otherwise it is a filled ribbon. Either way it takes the colour of its source segment, at `link_opacity` (default 0.55). Links that name a segment that does not exist are skipped.",
      ],
    },
  ],
  use: [
    "You have **long segments with positions on them**, and the relationships between positions on different segments are the point: [translocations](https://en.wikipedia.org/wiki/Chromosomal_translocation), gene fusions, module dependencies, cross-references.",
    "You want a quantity along each segment (density, coverage, error rate) next to the links that connect them. The tracks give you that in the same picture.",
    "There are a handful to a few dozen segments and up to a few hundred links.",
    "You want one figure that says the data is rich. It does that more convincingly than any other chart here, which is a mixed blessing.",
  ],
  avoid: [
    "You just have flows between groups and no positions on them. [Chord diagram](/charts/chord-diagram) is simpler.",
    "You have thousands of links. They will turn the middle into a solid disc. Filter to the significant ones or lower `link_opacity`.",
    "You need to compare values between tracks. Each track scales to its own maximum, so the heights are not comparable across tracks. A table or a [heatmap](/charts/heatmap) can say it better.",
    "Your segments are of very different lengths and the short ones matter. A segment 1% of the total is 3.6 degrees wide, and its links and bins will be very narrow.",
  ],
  read: [
    "The outer ring is the segments. Arc length is segment length, and ticks give the scale.",
    "The rings inside are the tracks. Within a ring, compare heights along the circle. Do not compare one ring's height with another's.",
    "The ribbons in the middle are the links. A wide ribbon is a long range. Its colour is the segment it starts from.",
    "Hover a segment for its length, a bin for its range and value, and a ribbon for both ends and its weight.",
  ],
  story: {
    title: "Twelve modules and a very old import",
    paragraphs: [
      "Birgit was the tech lead of a codebase at a logistics software company called Haulfast, and the codebase was fifteen years old. It had twelve top-level modules, a hundred thousand lines of Java in the largest, and a dependency structure that she described, on her better days, as organic.",
      "The company had decided to split the codebase into services, and she had been asked which modules could be moved first. The usual answer is the ones with the fewest dependencies on the others, and the usual way to find them is a graph with 3,000 edges, which in Birgit's experience looks like a bowl of noodles after a heated discussion.",
      "The facts she had were these: the size of each module in lines, the number of lines changed in each region in the last two years, and the list of imports between regions of different modules. In other words, segments, bins and links. That is a Circos plot with the names changed.",
      "She gave the lengths to `<Circos>` from bui-charts, one bin track of recent changes, and the 340 imports that crossed module boundaries as links, with the import count as weight. The twelve modules arranged themselves into a ring, with the biggest taking up a third of it.",
      "The outer track showed where the churn was: two hot regions in the order module, and almost nothing in the pricing module. The ribbons showed the rest. Pricing, quiet and stable, had only three links out of it. But a small, thick ribbon led from a region of the billing module to a spot near the start of the order module, and Birgit recognised it as an import of a utility class written in 2009 that nobody had dared touch.",
      "Pricing was the first module extracted. It took six weeks and the only surprise was the 2009 class, which was moved into a shared library and, for the first time in its life, given a test. The second to move was shipping, which the diagram had shown was equally quiet.",
      "Birgit has since taken to printing the circle on A3 and sticking it on the team wall. The team call the big ribbon Old Faithful, and nobody has asked her to explain the chart, which she takes as the highest compliment available.",
    ],
  },
  pitfalls: [
    "**Each track has its own scale.** The tallest bin in each track fills its ring. Never compare heights between tracks, and add the units to the caption.",
    "**Link ends must stay on their segments.** Positions are in the segment's unit. A position beyond the segment length draws past the end of its arc.",
    "**Segment indices.** Links and bins refer to segments by their index in the `segments` list, so reordering the list changes what they point to.",
    "**Overlapping ribbons hide each other.** With many links the centre saturates. Lower `link_opacity`, filter by weight or sort so important links draw last.",
    "**Small segments are hard to see.** A short segment gets a short arc, and a label that does not fit may overlap its neighbour.",
  ],
  related: [
    { slug: "chord-diagram", why: "When links join whole groups and there are no positions on them." },
    { slug: "arc-diagram", why: "When you have positions on a single line and want links drawn as arcs above it." },
    { slug: "sequence-logo", why: "When the data is aligned sequences and you want to see which letters are conserved at each position." },
  ],
  media: [
    { kind: "youtube", id: "Dv-ZObIy9QQ", title: "How to read a circos plot", channel: "OMGenomics", note: "Directly explains how to interpret a Circos plot, from a genomics perspective." },
    { kind: "youtube", id: "e_zjPY2vpaM", title: "How to make this circos plot in 1 minute", channel: "OMGenomics", note: "Quick demonstration of producing a Circos plot." },
  ],
}
