import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Plots the difference between two measurement methods against their average, with the limits within which they agree.",
  intro: [
    "You bought a cheaper blood-pressure cuff, or a faster assay, or a pocket ultrasound that fits in a lab coat. Now somebody asks the only question that matters: does it give the same answer as the old one? The tempting move is to scatter the new readings against the old ones and report a correlation. A correlation of 0.97 sounds wonderful, and it tells you almost nothing about agreement.",
    "Martin Bland and Douglas Altman made that argument in [a famous 1986 paper](https://doi.org/10.1016/S0140-6736(86)90837-8) in *The Lancet*, and the plot that came out of it carries both their names. The trick is to stop plotting one method against the other. Plot the **difference** between them on the vertical axis, and their **average** on the horizontal. Now disagreement is a visible quantity, with a level, a spread and a shape.",
    "Two methods can correlate perfectly while one reads ten units high on everything. The scatter plot calls that a success. This plot calls it a bias, which is the correct word.",
  ],
  math: [
    {
      heading: "Difference against mean",
      body: [
        "For each pair of readings $(a_i, b_i)$ of the same sample, the chart computes the mean and the difference. The point is drawn at $(m_i, d_i)$.",
      ],
      tex: ["m_i = \\frac{a_i + b_i}{2}, \\qquad d_i = a_i - b_i"],
    },
    {
      heading: "Bias and limits of agreement",
      body: [
        "The **bias** is the mean difference $\\bar d$: how far apart the methods are on average. The spread is the sample standard deviation of the differences, with the $n-1$ divisor. The [limits of agreement](https://en.wikipedia.org/wiki/Bland%E2%80%93Altman_plot) are $z$ spread-units either side of the bias, where $z$ is the `agreement` prop and defaults to 1.96.",
        "If the differences are roughly normal, about 95% of future differences between the two methods should land between the limits. Use `agreement={2.58}` for 99%.",
      ],
      tex: [
        "\\bar d = \\frac{1}{n}\\sum_i d_i, \\qquad s = \\sqrt{\\frac{1}{n-1}\\sum_i (d_i - \\bar d)^2}",
        "\\text{limits} = \\bar d \\pm z\\, s",
      ],
    },
    {
      heading: "What gets drawn",
      body: [
        "Differences are drawn as points. The bias is a solid neutral line and the two limits are dashed lines in the bad colour, all spanning from the smallest to the largest mean in the data. The legend names the limits by the multiplier you chose, such as +1.96 SD. Axis labels read \"Mean of A and B\" and \"A − B\", using your `a_label` and `b_label` when you give them.",
        "Note that the limits are flat. They assume the disagreement does not depend on the size of the measurement. If the points fan out to the right, that assumption is false, and the picture is telling you so.",
      ],
    },
  ],
  use: [
    "You are comparing a new instrument, assay or algorithm against an established one, on the same samples.",
    "You need to say how far apart two methods can be for one sample, in the units people actually use, not in correlation coefficients.",
    "You want to catch proportional error: disagreement that grows with the size of the measurement.",
    "You are comparing two raters or two repeated measurements and want a bias and a plausible range of differences.",
  ],
  avoid: [
    "You have measurements of different things. The chart only makes sense when both numbers are measuring the same quantity.",
    "One of the methods is a true reference and you want calibration. A regression of one on the other is a better fit for that job.",
    "The differences are clearly not roughly normal, for instance heavily skewed. The 1.96 multiplier will mislead. Transform the data first, or use percentiles.",
    "There are only a handful of pairs. Limits from six samples are very uncertain.",
  ],
  read: [
    "Find the solid bias line. If it sits well away from zero, one method reads consistently higher than the other.",
    "Look at the two dashed lines. The distance between them is the spread you must be willing to tolerate for the methods to be interchangeable. That is a clinical or engineering decision, not a statistical one.",
    "Check the shape of the cloud. A flat band is good. A fan or a slope means the disagreement depends on the size of the value.",
    "Look for isolated points beyond the limits. A few are expected. Many means the normal assumption is wrong.",
    "Hover any point to read its difference and its average.",
  ],
  story: {
    title: "Correlation 0.97, bias eleven",
    paragraphs: [
      "Priya was a clinical scientist at a hospital lab whose haematology analyser was ageing in the way equipment does, loudly and with opinions. The hospital had a quote for a new point-of-care device that gave haemoglobin results in four minutes instead of forty, and the procurement committee wanted a one-page justification.",
      "She ran eighty patient samples on both machines. Her first plot was the usual one: old device on the x axis, new device on the y axis, a line through it. The correlation came out at 0.97. She printed it, looked at it, and had the uneasy feeling that it was too pleasant.",
      "The trouble with a scatter plot is that the eye reads the line, and the line says yes. It cannot show that the new device sat about 11 g/L above the old one in the middle of the range.",
      "So she redrew the same eighty pairs with `<BlandAltman>`, with the analyser as `a` and the new device as `b`. The picture was blunt. The bias line sat at −11, and the limits of agreement ran from about −24 to +2. A patient reading 105 on the new device would probably read anywhere from 81 to 107 on the old analyser.",
      "The points also fanned out to the right. At low haemoglobin, where a transfusion decision is made, the two devices agreed within a few g/L. At high values they drifted apart by twenty.",
      "The committee did not reject the device. They approved it for emergency triage at the bedside, where the low range is what matters, and kept the lab analyser for everything else. The recalibration the vendor shipped three months later pulled the bias to −2.",
      "Two instruments can be best friends in correlation and strangers in practice. This chart asks the rude question politely.",
    ],
  },
  pitfalls: [
    "**Reporting r instead of the plot.** Correlation measures how well one method predicts the other up to a straight line. It does not measure whether they agree.",
    "**Treating limits as tolerances.** The chart computes them from your data. Whether a spread of 22 g/L is acceptable depends on the decision being made.",
    "**Ignoring the fan.** If differences grow with the mean, flat limits are too narrow for large values and too wide for small ones. Consider plotting ratios or log values.",
    "**Argument order.** The difference is `a − b`. Swap the methods and the plot flips top to bottom. Pick the reference as `b` and keep the order consistent.",
  ],
  related: [
    { slug: "xy-chart", why: "For the plain scatter of one method against the other, which you can add beside this to show correlation." },
    { slug: "control-chart", why: "Another chart built on a centre line and standard-deviation limits, but over time." },
    { slug: "forest", why: "When the comparison is across many studies and you need intervals for each." },
  ],
  media: [
    { kind: "youtube", id: "hHsfn7jE79M", title: "Bland-Altman Plot [Simply explained]", channel: "numiqo", note: "Clear explanation of bias and limits of agreement and how to read the plot." },
    { kind: "youtube", id: "rXFHZOMuYQA", title: "Introduction to the Bland-Altman Plot", channel: "Zach's Math Zone", note: "Short introduction to constructing and interpreting the plot." },
  ],
}
