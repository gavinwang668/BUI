import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Counts of values per bucket, so you can see the shape of a whole distribution at once.",
  intro: [
    "An average is a summary, and summaries are polite. They tell you what the data would like you to believe. A histogram is the data in its own clothes: take every value, drop it into a bucket by size, count the buckets, draw the counts as bars. What comes out is a shape, and the shape tells you things the mean never would. One hump or two. A long tail or a cliff. A suspicious spike at exactly zero.",
    "[Karl Pearson](https://en.wikipedia.org/wiki/Karl_Pearson) gave the histogram its name in the 1890s, and the idea of counting things into bins is of course far older. The chart has outlived most of the fashions that tried to replace it, for the dull reason that it is hard to fool. You cannot hide a [bimodal distribution](https://en.wikipedia.org/wiki/Multimodal_distribution) in a histogram, though people have tried.",
    "The one decision that is yours is the bucket size. Too wide and everything merges into one hump. Too narrow and the hump becomes a comb. In bui-charts the default aims for about thirty buckets and rounds the width to a friendly number, so you get a sensible first picture and can decide whether the buckets mean something afterwards.",
  ],
  math: [
    {
      heading: "Binning",
      body: [
        "Given values $x_1, \\dots, x_n$, a bucket width $w$ and an offset $o$ (zero by default), the bucket of a value is identified by its lower edge, and the bucket covers the half-open interval from that edge up to the edge plus $w$. The count of a bucket is the number of values whose lower edge matches it.",
        "Only buckets that contain at least one value are produced, so gaps in the data show as gaps. Lower edges are rounded to nine decimals to keep floating-point crumbs out of the bucket identity, which stops 0.30000000000000004 from becoming a bucket of its own.",
      ],
      tex: [
        "\\ell(x) = w \\left\\lfloor \\frac{x - o}{w} \\right\\rfloor + o, \\qquad c_k = \\#\\{\\, i : \\ell(x_i) = \\ell_k \\,\\}",
      ],
    },
    {
      heading: "Choosing the width",
      body: [
        "When you give neither `bucket_size` nor `bucket_count`, the chart aims for $n = 30$ buckets. It computes the target width as the range of all values divided by $n$, and then picks the smallest width from a ladder of round numbers (1, 2, 2.5, 4, 5 times a power of ten, from $10^{-9}$ to $10^{9}$) that is larger than the target and also no smaller than the smallest gap between distinct values.",
        "The second condition is what keeps integer data tidy: if your values are whole numbers, the bucket will never be narrower than 1, so no bucket is mysteriously empty. `bucket_offset` set to half the width centres buckets on round numbers.",
      ],
      tex: [
        "w = \\min\\{\\, s \\in \\mathcal{S} : s > \\tfrac{\\max x - \\min x}{n},\\ s \\ge \\delta_{\\min} \\,\\}",
      ],
    },
    {
      heading: "Several series and pre-counted data",
      body: [
        "With several series, values from all of them are bucketed with the same width and offset, so bars line up. Each bucket row then has one count per series. `stacking` stacks them to show the total and the parts, `combine={true}` merges everything into one count, and an overlay with lower `fill_opacity` compares shapes.",
        "If your data is already counted, you pass the lower and upper edges and a count. A missing edge is guessed from the spacing of the other one, and non-finite counts are treated as zero.",
      ],
    },
  ],
  use: [
    "You want the **shape** of one numeric variable: spread, skew, modes, outliers.",
    "You are comparing two groups and want to see whether their shapes differ, not only their means.",
    "You are deciding how to summarise. A long tail argues for percentiles. Two humps argue for splitting the data first.",
    "You have raw samples (request latency, file sizes, test scores) or counts already in buckets from a metrics system.",
  ],
  avoid: [
    "You want how the distribution changes over time. A [Heatmap](/charts/heatmap) with `calculate` stacks one histogram per time bucket.",
    "You have very few values. Twelve values in thirty buckets is a drawing of twelve bars.",
    "You need to compare each value against another variable. [XY Chart](/charts/xy-chart).",
    "You want precise quantiles. A histogram shows them roughly. Compute them.",
  ],
  read: [
    "Find the tallest bar, the mode, and ask whether it is the only one. A second hump means two populations.",
    "Look at the tails. A long right tail means a few large values pull the mean away from the typical one.",
    "Check the bucket width before you read anything into the bar heights. Different widths make heights incomparable.",
    "Hover a bar for its range and count. With several series, overlap and stacking behave differently, so check which one you are looking at.",
  ],
  story: {
    title: "The exam that had two peaks",
    paragraphs: [
      "Oksana taught introductory statistics at a technical college in Lviv, and every term she graded the final exam, computed the mean, and posted it. Sixty-one out of a hundred, again, which felt right and told her nothing at all.",
      "This term the head of department asked her why a third of the class had still failed, given that the average looked healthy. It was a reasonable question, and she had no answer, because she had never actually looked at the scores. She had looked at their mean, which is a different thing.",
      "She exported the 214 results into a spreadsheet and plotted them with bui-charts, using `<Histogram>` with a bucket size of five points and a single series. She expected a hill with a hump at sixty. She wanted to be right, in the same way that everyone wants to be right.",
      "The chart had two humps. One at about 40 and one at about 80, with a valley between them at 60 that was very nearly empty. Sixty-one was the average of two groups, and almost nobody had actually scored sixty-one.",
      "She split the data by which tutorial group the student had attended. The two humps separated cleanly: a section that had been taught by a substitute for the middle four weeks had scored low, and the rest high. The lecturer on leave had covered conditional probability, which is the one topic on the exam that is entirely made of ideas you only get once.",
      "Oksana ran two revision sessions for that section on conditional probability. At the retake, the low hump had moved from 40 to 66 and the valley had filled in.",
      "She now plots the histogram before she computes anything else. The mean, she tells her students, is the number you get when you ask a crowd to stand in the middle of a room, and nobody in the room is standing there.",
    ],
  },
  pitfalls: [
    "**Bucket width changes the story.** Try a coarser and a finer width before you conclude there is a gap or a second hump.",
    "**Unequal widths.** Pre-counted buckets of different sizes make bar heights misleading. Count per unit width or use equal buckets.",
    "**Overlaying more than two series.** Overlapping bars turn to mud. Stack them, or compare two at a time.",
    "**Integers and half-open buckets.** Edges belong to the bucket above, so a value of exactly 10 falls in 10 to 15, not 5 to 10.",
    "**Treating the histogram as a density.** Bars show counts, not probabilities, so the totals depend on how much data you have.",
  ],
  related: [
    { slug: "heatmap", why: "One histogram per time bucket, drawn as colour, to see how the shape changes." },
    { slug: "ridgeline", why: "Several distributions drawn as overlapping curves, one per group." },
    { slug: "barcode", why: "Every value as a tick, when you have few samples and want to see each one." },
    { slug: "beeswarm", why: "Every point shown without overlap, per category." },
  ],
  media: [
    { kind: "youtube", id: "qBigTkBLU6g", title: "StatQuest: Histograms, Clearly Explained", channel: "StatQuest with Josh Starmer", note: "Well-known, clear explanation of histograms and bin width." },
  ],
}
