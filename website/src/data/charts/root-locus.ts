import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "The paths the closed-loop poles take across the complex plane as the feedback gain is turned up.",
  intro: [
    "You close a feedback loop around a system, you turn the gain up to make it respond faster, and at some point it begins to ring, then to shake, then to leave the building. Every control engineer has seen it happen and wants to know where. The location is not mysterious: it is the point where a closed-loop pole crosses from the left half of the complex plane into the right. The root locus is a map of every place those poles go on the way.",
    "[Walter Evans](https://en.wikipedia.org/wiki/Root_locus_analysis) introduced the method in 1948, in an era when the alternative was solving a polynomial by hand for each gain you cared about, and a graphical trick that let you sketch all the answers at once was a gift. It still is. A root locus shows you stability, speed and damping for every gain in one picture, and it tells you whether a gain knob can fix your problem at all or whether you need to change the plant.",
    "Poles start at the open-loop poles when the gain is zero. As the gain grows they travel, and in the limit they end at the zeros, or run to infinity. The chart draws each path in its own colour, so you can watch the rise of an instability as one branch heading for the imaginary axis.",
  ],
  math: [
    {
      heading: "The closed-loop equation",
      body: [
        "Take an open-loop [transfer function](https://en.wikipedia.org/wiki/Transfer_function) $G(s) = N(s)/D(s)$ and unity negative feedback with gain $K$. The closed-loop poles are the roots of the characteristic equation below. Pass `numerator` and `denominator` as coefficient lists, highest power first, so $s^{2}+3s+2$ is `vec![1., 3., 2.]`.",
      ],
      tex: [
        "1 + K\\,\\frac{N(s)}{D(s)} = 0 \\quad\\Longleftrightarrow\\quad D(s) + K\\,N(s) = 0",
      ],
    },
    {
      heading: "Sweeping the gain",
      body: [
        "The chart takes `steps` values of $K$ (400 by default) across the `gain` range, which is $[0, 100]$ unless you say otherwise. The spacing depends on the lower end. If it is positive, the values are geometric, so each decade of gain gets equal room, if it is zero or negative, they follow a quadratic ramp that puts more samples at small gain, where the poles move fastest. With $t$ running from 0 to 1:",
      ],
      tex: [
        "K(t) = K_{\\min}\\left(\\frac{K_{\\max}}{K_{\\min}}\\right)^{t} \\quad (K_{\\min} > 0), \\qquad K(t) = K_{\\min} + (K_{\\max} - K_{\\min})\\,t^{2} \\quad (K_{\\min} \\le 0)",
      ],
    },
    {
      heading: "Finding the roots and keeping branches straight",
      body: [
        "At each $K$ the two polynomials are padded to the same length, combined coefficient by coefficient into $D + K N$, and the roots are found with the [Durand-Kerner](https://en.wikipedia.org/wiki/Durand%E2%80%93Kerner_method) iteration: start from $n$ points on the powers of $0.4 + 0.9i$ and repeatedly move each estimate by $p(z_i)/\\prod_{j\\ne i}(z_i - z_j)$, for up to 500 sweeps or until no estimate moves more than $10^{-12}$.",
        "Root finders return their answers in no particular order, so a naive plot would swap branches at random. The chart matches each new root to the *nearest* root of the previous gain, which keeps each colour attached to its own path. Poles are drawn as red squares at the open-loop roots, zeros as green hollow circles. The system is stable at a given $K$ if every root has a negative real part, which on the plot means every point is left of the vertical axis.",
      ],
    },
  ],
  use: [
    "You are choosing a gain for a loop and want to know how far you can push it before it goes unstable.",
    "You want to see how adding a pole or zero, such as a lag or lead compensator, bends the paths and changes what gain can do.",
    "You want to read damping and settling speed off the geometry: how far the poles are from the imaginary axis and how far off the real axis.",
    "You are teaching: this is the one chart that explains why high gain is not free.",
  ],
  avoid: [
    "Your loop has a large pure time delay. A delay is not a rational function, so it must be approximated before the locus is meaningful.",
    "You need to measure a real system's margins. A Nyquist plot or a Bode plot is based on measured frequency response, not on a model.",
    "You have several tunable parameters. A root locus is a one-parameter sweep. Two parameters need a family of them.",
    "The model is badly conditioned or very high order. Root finding on a polynomial of degree 20 or more loses accuracy quickly.",
  ],
  read: [
    "Find the squares (poles) and circles (zeros). At the lowest gain, each branch begins at a square.",
    "Follow each branch. It ends at a circle or runs off the edge, and the colour tells you which branch you are following.",
    "Note where any branch crosses the vertical axis. That is the gain at which the loop becomes unstable. Hover near the crossing to read the coordinates.",
    "Look for branches that leave the real axis and curve: that is the gain where the response starts to oscillate, and the angle from the axis is the damping.",
  ],
  story: {
    title: "The antenna that learned to wobble",
    paragraphs: [
      "Hamid was a control engineer at a small company that built tracking antennas for weather satellites. The dish had to follow a pass across the sky within a tenth of a degree, and the motor loop was tuned by a person named Roy, who had retired and left a note that said, in full, gain of 12, do not touch.",
      "Customers wanted faster tracking. A faster tracker meant a higher gain, and Hamid had a model of the dish, a motor with a slow mechanical pole and a faster electrical one, along with a deadline and a mild fear of Roy's note. He could try a higher gain on the real antenna and learn the limit the hard way, which is also how the previous generation of dishes had learned it, loudly, in the car park.",
      "He wanted the limit on paper first. He also wanted to know whether the gain knob was even the right tool, or whether the dish needed a compensator before any amount of gain could give the speed the customers wanted.",
      "In bui-charts he passed the model's numerator and denominator to `<RootLocus>` with a gain range from 0.1 to 200. Two branches left the real axis near the origin, met in the middle, split into a complex pair and curved to the right. They crossed the imaginary axis at a gain of about 38. Roy's 12 was comfortably inside, with a pair of poles at a damping ratio of 0.6.",
      "That gave him the answer and the problem. The dish could go to a gain of about 25 with acceptable damping, and no further. Past that, the arc curved too close to the axis. A faster loop needed the poles moved, not just pushed, so he added a lead compensator: a zero at minus 4 and a pole at minus 20. The new locus bent the branches away from the axis and gave him a gain of 70 before instability.",
      "The new dish tracked a fast pass with a worst-case pointing error of 0.04 degrees, down from 0.11. Roy was invited to the demonstration and stood in front of the plot for a full minute. He said his gain of 12 had been the right answer, for the question he was asked.",
    ],
  },
  pitfalls: [
    "**Coefficient order.** Highest power first. Reversing the list gives you the locus of a different system, usually one that looks plausible.",
    "**Numerator longer than denominator.** An improper transfer function has no valid locus. The chart will draw something, and it will mean nothing.",
    "**A gain range that starts at zero.** The quadratic ramp is good for seeing the poles leave their start, but gives few samples at large gain. If you care about the far end, start above zero to get geometric spacing.",
    "**Branch swapping at a collision.** Where two branches meet on the real axis, nearest-neighbour matching can exchange them for one step. The paths are right as a set. The colours at the junction are not guaranteed.",
  ],
  related: [
    { slug: "smith-chart", why: "When the question is impedance matching across frequency rather than loop stability across gain." },
    { slug: "phase-portrait", why: "When you want to see the behaviour of a nonlinear system in the state plane, not the poles of a linear one." },
    { slug: "oscilloscope", why: "When you want to see the step response of the loop you just tuned, as a waveform." },
  ],
  media: [
    {kind:"youtube",id:"4cMHfTsPz3M",title:"What is Root Locus Plot?",channel:"Engineering Funda",note:"Introduces the root locus and what its branches say about closed-loop stability."},
  ],
}
