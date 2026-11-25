import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "Stacked, overlapping density curves, one per group, on a shared axis.",
  intro: [
    "Comparing the distribution of one number across twelve groups is an awkward job. Twelve histograms side by side are too small to read. Twelve overlaid on one set of axes are a spaghetti plate. Twelve box plots show you five numbers each and ask you to imagine the rest. The ridgeline plot's answer is to draw each group as a smooth curve, give each its own row, and let the rows overlap a little so the page is used well.",
    "The most famous ridgeline in the world is on a record sleeve: the cover of Joy Division's [*Unknown Pleasures*](https://en.wikipedia.org/wiki/Unknown_Pleasures) shows successive radio pulses from the [pulsar CP 1919](https://en.wikipedia.org/wiki/PSR_B1919%2B21), stacked in a pile of white lines on black. The picture came out of a 1970 astronomy thesis and became the shorthand for this layout, which is why the chart is also known as a joyplot. Ridgeline is the more neutral name and the one this library uses.",
    "The pleasure of the chart is in the comparison. Because every curve shares the same horizontal axis, you can run an eye down one value and see which groups put weight there. Shifts, spreads and second humps appear as a landscape, not as a table.",
  ],
  math: [
    {
      heading: "A kernel density estimate per group",
      body: [
        "For a group with values $x_1, \\dots, x_n$ the curve is a Gaussian [kernel density estimate](https://en.wikipedia.org/wiki/Kernel_density_estimation). At every point $t$ of the grid it adds a small bell centered on each observation and averages them, which makes the curve a smoothed version of the histogram with no bins to choose.",
        "The curve integrates to 1 over the whole line, so each group contributes the same area regardless of its size. Non-finite values are ignored.",
      ],
      tex: ["\\hat f(t) = \\frac{1}{n\\,h\\,\\sqrt{2\\pi}} \\sum_{i=1}^{n} \\exp\\!\\Bigl(-\\tfrac12\\bigl(\\tfrac{t - x_i}{h}\\bigr)^{2}\\Bigr)"],
    },
    {
      heading: "Bandwidth",
      body: [
        "The bandwidth $h$ is the width of each little bell, and it is the one decision that changes the picture. By default every group gets its own, from Silverman's rule of thumb, using the sample standard deviation $s$ of that group: $h = 1.06\\, s\\, n^{-1/5}$. If a group has fewer than two values, or all values equal, the fallback is $h = 1$.",
        "That rule is tuned for bell-shaped data. For data with several peaks it oversmooths and the humps merge. Pass a smaller `bandwidth` (one value, applied to every group) to separate them. Too small and each point becomes its own spike.",
      ],
      tex: ["h = 1.06\\, s\\, n^{-1/5}"],
    },
    {
      heading: "From curves to rows",
      body: [
        "All curves are evaluated on one grid of 128 points that runs from the smallest value minus $3h_{\\max}$ to the largest value plus $3h_{\\max}$, where $h_{\\max}$ is the largest bandwidth in use; that margin is where the tails fall to zero. The curves are then divided by the single tallest density across all groups, and multiplied by `overlap` (default 1.6).",
        "The consequence is that all groups share one vertical scale: a curve twice as tall holds twice the density, not just a bigger sample. The baseline of group $g$ of $G$ is at height $G - 1 - g$, so the first group in your data is the top row, and the tallest curve reaches 1.6 rows above its baseline and into the row above it.",
      ],
      tex: ["y_g(t) = (G - 1 - g) + \\text{overlap}\\cdot\\frac{\\hat f_g(t)}{\\max_{g,t} \\hat f_g(t)}"],
    },
  ],
  use: [
    "You compare the shape of one measure across many groups: salaries by department, temperatures by month, scores by school.",
    "The groups have a natural order (months, years, doses), so the stack reads as a story of change.",
    "Group sizes are large enough for a smooth estimate: dozens of values or more per group.",
    "You want a picture that is compact and a bit beautiful, and you have ten or twenty groups.",
  ],
  avoid: [
    "Groups have very few values. A kernel estimate of five points is a guess in a nice dress. Show the points with a [beeswarm](/charts/beeswarm) or [barcode](/charts/barcode).",
    "You care about exact values or counts. The curve is a density, not a tally, and the heights are relative.",
    "Your data has hard limits (non-negative amounts, percentages). The kernel spills past the limit and shows density where no value can exist.",
    "You have only two or three groups. Overlaid or side-by-side [histograms](/charts/histogram) are easier to compare.",
  ],
  read: [
    "Read the horizontal position of a peak as the typical value for that group.",
    "Read the width of a curve as spread: a narrow hump is a consistent group, a wide one is a varied group.",
    "Because all curves share a scale, a taller curve is a more concentrated one. Compare heights as well as positions.",
    "Look for second humps. They usually mean two populations hiding in one group.",
    "Tooltips are off by default, because the picture is about shape. Pass `tooltip` if you need one.",
  ],
  story: {
    title: "Nine months of Wednesday",
    paragraphs: [
      "Lena worked on the data team of a bike-share system in a hilly city, and she had been asked a question that sounded simple: do trips get longer in summer? Her manager wanted one chart for a quarterly review, and had said, hopefully, that it could be \"something nice\".",
      "She had 600,000 trips across nine months. A line of average duration by month was easy to make and told her nothing. It moved by about 40 seconds and the manager would conclude that nothing happened. A histogram for each month was accurate and nine panels tall.",
      "The thing she suspected was not a shift in the average at all. She thought the *shape* was changing: more short commuter hops in winter, and a second, longer group of leisure rides arriving in spring. Averages cannot show that, and she could not show it with nine histograms in a slide that would be on screen for forty seconds.",
      "She put a sample of 2,000 trips per month into a `<Ridgeline>` from bui-charts, grouped by month in calendar order, and left the bandwidth alone. One chart, one row per month, one shared axis from 0 to 60 minutes.",
      "January through March were a single sharp peak at around 8 minutes. In April a low shoulder appeared at 25 minutes. By June it was a proper second hump, and in July it was almost as tall as the first. It looked like a mountain range being born, which several people in the review mentioned, and one of them asked whether it was the Joy Division cover. It was not, but she did not argue.",
      "The finding changed a decision. The ops team had been sizing summer rebalancing by average trip length, and so was leaving the stations near the lakefront empty by mid-afternoon. They re-planned around the second hump, and the number of empty-dock alerts at those stations dropped from 310 in July to 120 in August.",
      "Lena's manager said the chart was nice. This was the first time in her career that nice and useful had been the same word.",
    ],
  },
  pitfalls: [
    "**Oversmoothing.** The default bandwidth assumes one bell. If a group has two peaks, pass a smaller `bandwidth`, otherwise the second one is melted away.",
    "**Smoothing past a boundary.** A distribution that cannot go below zero will show curve to the left of zero. The kernel does not know about your limits.",
    "**Reading overlap as data.** Curves reach into the row above because `overlap` says so. A curve that covers part of its neighbour is not hiding anything underneath, but a tall one can hide a short one: lower `overlap` when it does.",
    "**Tiny groups look as confident as large ones.** Every curve is a smooth shape whatever the sample size. Show group sizes in the labels if they differ a lot.",
  ],
  related: [
    { slug: "histogram", why: "When counts per bucket are what you need, and for one or two groups." },
    { slug: "beeswarm", why: "When groups are small and every observation should be visible." },
    { slug: "horizon", why: "When the thing that changes across rows is a time series, not a distribution." },
  ],
  media: [
    { kind: "youtube", id: "kU4O1LXdz2Y", title: "How to create a ridgeline plot in R with ggridges in RStudio (CC226)", channel: "Riffomonas Project", note: "Builds a ridgeline step by step and discusses overlap and bandwidth, the two settings that matter." },
    { kind: "youtube", id: "GCTsRLJ1dfU", title: "Ridgeline Plot in Python", channel: "Bhavesh Bhatt", note: "A short Python version for comparing the distribution of one number across several groups." },
  ],
}
