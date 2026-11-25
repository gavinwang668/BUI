import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A time series with a centre line and control limits that tell you when a point is more than noise.",
  intro: [
    "Every process wobbles. Fill-weights vary, response times drift, the coffee machine produces a slightly different coffee every morning. The hard question is not *did it change?* but *did it change more than it usually does?* Without an answer, you either chase every blip, which exhausts everybody, or ignore every shift, which ruins the product.",
    "The control chart is [Walter Shewhart](https://en.wikipedia.org/wiki/Walter_A._Shewhart)'s answer, from his work at Bell Labs in the 1920s. Draw the measurements in time order. Add a line at the average. Add two more lines three standard deviations either side. Points inside the lines are what the process does when nothing special is happening. A point outside is a reason to go and look.",
    "It is a surprisingly polite instrument. It never tells you what went wrong, only that the process has stopped behaving like itself, and it never raises its voice unless it has to.",
  ],
  math: [
    {
      heading: "Centre, spread, limits",
      body: [
        "Take observations $x_1, \\dots, x_n$ in time order. The centre line is the mean, unless you pass `center`. The limits sit $L$ standard deviations either side of it, where $L$ is `limit` and defaults to 3.",
        "The spread is *not* the ordinary standard deviation of all the points. If the process has shifted, the ordinary standard deviation absorbs the shift and the limits balloon around the very problem you want to see. Instead, `sigma` is estimated from the [moving range](https://en.wikipedia.org/wiki/Shewhart_individuals_control_chart) of consecutive points, which only sees short-term wobble.",
      ],
      tex: [
        "\\overline{MR} = \\frac{1}{n-1} \\sum_{i=2}^{n} \\lvert x_i - x_{i-1} \\rvert, \\qquad \\hat\\sigma = \\frac{\\overline{MR}}{d_2}, \\quad d_2 = 1.128",
        "\\mathrm{UCL} = \\bar{x} + L\\hat\\sigma, \\qquad \\mathrm{LCL} = \\bar{x} - L\\hat\\sigma",
      ],
    },
    {
      heading: "Why 1.128",
      body: [
        "For two independent normal values with standard deviation $\\sigma$, the expected absolute difference is $d_2\\sigma$ with $d_2 \\approx 1.128$. Dividing the mean moving range by it therefore recovers $\\sigma$ for a stable process. Non-finite values are skipped before consecutive pairs are formed.",
        "For a normal process, a point beyond three sigma happens with probability about $0.0027$, so a stable process raises a false alarm about once every 370 points. That number, not any rule of nature, is the reason three was picked.",
      ],
      tex: ["P(\\lvert Z \\rvert > 3) \\approx 0.0027 \\;\\Rightarrow\\; \\text{average run} \\approx 370"],
    },
    {
      heading: "What gets drawn",
      body: [
        "Each row is plotted at its x value. A point is flagged as out of control when it is strictly above the UCL or strictly below the LCL, and the flagged points are redrawn as larger markers in the theme's bad colour. With `zones={true}` four faint dotted lines are added at $\\pm 1\\hat\\sigma$ and $\\pm 2\\hat\\sigma$, for [run rules](https://en.wikipedia.org/wiki/Nelson_rules) such as eight points in a row on one side of the centre.",
        "Passing `center` and `sigma` replaces the estimates, so new data is judged against limits learned earlier instead of against itself. The x axis defaults to time.",
      ],
    },
  ],
  use: [
    "You monitor a process over time and need a rule for when to react: line fill weights, batch yields, page load times, nightly job durations.",
    "You are deciding whether an improvement did anything. Compare the points after the change against the old limits.",
    "You want to stop a manager from reacting to individual points. Showing them the limits is cheaper than explaining variation.",
  ],
  avoid: [
    "The data has no meaningful time order, such as a set of unrelated samples. A [histogram](/charts/histogram) describes those better.",
    "The process has a strong trend or seasonal cycle. The limits will flag the cycle as a crisis. Remove the pattern first, or chart the residuals.",
    "You have fewer than about twenty points. The limits are mostly guesswork at that size.",
    "You need to compare two measurement methods. That is a different question. Use a [Bland-Altman plot](/charts/bland-altman).",
  ],
  read: [
    "Follow the line left to right. Ignore any single wiggle inside the limits, that is the process being itself.",
    "Look for red points. They fall outside the dashed limit lines and deserve an investigation.",
    "Look for patterns inside the limits too: a long run on one side of the centre, or a steady drift, means the mean has moved even if no point is out.",
    "With zones on, count how many points sit beyond the 2-sigma lines. Several in a row is a warning before the limit is ever crossed.",
    "Hover a point for its value, the centre and both limits.",
  ],
  story: {
    title: "The Thursday that was just Thursday",
    paragraphs: [
      "Idris ran operations for a food-delivery startup whose promise was thirty-minute delivery, or at least a heartfelt apology. Every Monday he received a message from the founder that began with the median delivery time for the previous day and ended with several exclamation marks of varying emotional temperature.",
      "On a Thursday in October the median was 33.1 minutes, up from 31.4. By nine in the morning the founder had proposed new rider bonuses, a different routing vendor and, in a message sent at 9:14, a vow to \"completely rethink logistics\".",
      "Idris suspected the median had done nothing more than be a number. But suspecting is a weak argument against a man who has already drafted the vendor email.",
      "He exported ninety days of daily medians and drew them with `<ControlChart>`, letting it estimate the centre and the sigma from the data. The centre came out at 31.9 minutes and the upper limit at 35.2. Thursday sat comfortably inside, a little above the line, in the company of eleven other days that had looked equally alarming at the time.",
      "He sent the picture with one sentence: the process had not changed, and the limit was 35.2. The vendor email stayed in drafts.",
      "Three weeks later a point did poke through the top. That one was real: a mapping update had silently sent riders to the wrong side of a river. It was found in a day, partly because nobody had spent the previous month crying wolf at ordinary Thursdays.",
      "A control chart's main job is not to find problems. It is to make sure you only get excited when it counts.",
    ],
  },
  pitfalls: [
    "**Limits from contaminated data.** Estimating limits from a series that already contains a shift gives you limits that hide it. Once you know the process, pass `center` and `sigma` from a stable stretch.",
    "**Confusing control limits with specification limits.** Control limits describe what the process does. A spec describes what the customer wants. A process can be perfectly stable and perfectly useless.",
    "**Autocorrelated data.** If each point depends on the last (a slowly draining tank, a rolling average), consecutive differences are small and the limits become too tight and cry wolf. Chart raw, independent measurements where possible.",
    "**Reacting to every red dot.** About one stable point in 370 will be outside at three sigma by pure bad luck. Investigate, but do not fire anyone.",
  ],
  related: [
    { slug: "time-series", why: "When you just want to see the series with annotations and no statistical judgement." },
    { slug: "pareto", why: "After a signal, to rank which defect types are driving it." },
    { slug: "bland-altman", why: "When the question is whether two instruments agree, not whether one process is stable." },
  ],
  media: [
    { kind: "youtube", id: "iOJDBpOa3Ko", title: "Control Charts simply explained - Statistical process control - Xbar-R Chart, I-MR Chart,...", channel: "numiqo", note: "Explains control limits and the common control chart types." },
    { kind: "youtube", id: "aey2c0VsZNc", title: "Statistical Process Control (SPC) Explained | Control Charts in Quality Management", channel: "Operations & Supply Chain Management University", note: "Places control charts in the wider SPC practice." },
  ],
}
