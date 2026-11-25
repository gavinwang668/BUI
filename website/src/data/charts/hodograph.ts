import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "The tip of a wind vector traced from the ground upward, drawn on rings of speed around calm.",
  intro: [
    "Imagine standing on the ground and holding a bundle of arrows, one for the wind at each height. Now slide every arrow so its tail sits at the same spot, the origin, and mark where each tip lands. Join the marks from the ground up. That curve is a [hodograph](https://en.wikipedia.org/wiki/Hodograph), and it is one of the few charts where a single line holds a whole atmosphere's worth of wind.",
    "The idea is older than weather forecasting. The mathematician [William Rowan Hamilton](https://en.wikipedia.org/wiki/William_Rowan_Hamilton) introduced the word in the 1840s for the curve traced by the tip of a velocity vector, and it has been quietly useful in orbital mechanics and fluid dynamics ever since. Meteorologists adopted it because storms care about *change* in wind with height, and the hodograph shows that change as a shape you can see: a straight line is steady [shear](https://en.wikipedia.org/wiki/Wind_shear), a curve is turning, and a hook is the kind of thing that makes a forecaster stop eating.",
    "Reading one is mostly about distance and angle from the middle. The centre is calm. A point two rings out is twice as windy as a point on the first ring. The direction of the point from the centre is where the wind blows toward. After that, the chart gets out of your way and lets the path be the story.",
  ],
  math: [
    {
      heading: "A vector path, not a function of x",
      body: [
        "Each row is a level with east and north components $(u_i, v_i)$, listed from the ground up. The chart plots exactly those coordinates, joined in row order. Speed and direction are recovered from position:",
      ],
      tex: [
        "s_i = \\sqrt{u_i^2 + v_i^2}, \\qquad \\theta_i = \\operatorname{atan2}(v_i,\\, u_i)",
      ],
    },
    {
      heading: "Speed rings",
      body: [
        "The rings are circles centred on calm, one per multiple of a step $\\Delta$. If you do not pass `ring_step`, $\\Delta$ is a round number (1, 2, 5 or 10 times a power of ten) near a quarter of the fastest wind $s_{\\max}$ in the data. The number of rings is chosen so the last one is at least as far out as the fastest wind, and the ring set is always at least one ring.",
      ],
      tex: [
        "\\Delta \\approx \\frac{s_{\\max}}{4}, \\qquad N = \\max\\!\\left(1,\\; \\left\\lceil \\frac{s_{\\max}}{\\Delta} \\right\\rceil\\right), \\qquad r_n = n\\,\\Delta \\;\\; (n = 1,\\dots,N)",
      ],
    },
    {
      heading: "Equal axes, coloured levels",
      body: [
        "Both axes always share one scale and run symmetrically through zero, so the rings stay round. A circle that is squashed into an oval would make the same wind look stronger toward one direction, which is the one lie this chart cannot afford.",
        "The points on the path are tinted by the `height` value from the cool end of the scheme to the warm end. If you do not give a height, the row index is used, so the colour still tells you which end of the profile you are on. The default unit label is knots, and the axes are titled $u$ and $v$.",
      ],
    },
  ],
  use: [
    "You have **wind (or any 2D vector) as it changes along a line**: with height, with time, along a river, along a flight.",
    "You care about **shear and turning**, not only the speed. Storm forecasting is the classic case.",
    "You want to compare the full profile with a threshold ring, for instance whether the wind ever passes 50 knots.",
    "You need a single compact picture of a whole sounding to sit beside a thermodynamic diagram.",
  ],
  avoid: [
    "You want to read the wind at an exact height. A hodograph hides the height axis in the colour, so a table or wind barbs are better for exact levels.",
    "Your rows are not ordered along the path. The line is joined in row order, so shuffled data draws a tangle.",
    "You have vectors at many places and one time. That is a map of arrows, which is what a [quiver](/charts/quiver) is for.",
    "Your audience has never seen one. Give them the speed rings and a sentence on what the centre means.",
  ],
  read: [
    "Start at the first point, the lowest level, and follow the line outward. The colour tells you the height as you go.",
    "Read speed as distance from the centre against the rings. Read direction as the compass bearing of the point from the centre, remembering it is where the wind is going toward in $(u, v)$.",
    "A straight segment means the wind changes steadily with height. A curve that bends clockwise is wind veering with height, which is the usual signature of a storm-friendly environment.",
    "Hover a point for its components and its height value.",
  ],
  story: {
    title: "Forty knots, in the wrong direction",
    paragraphs: [
      "Soo-jin was a launch meteorologist for a small company that sent sounding rockets up from a coastal range, and the launch window was a polite fiction. On paper it was 2 to 4 p.m. In practice the range officer said yes or no based on a sheet of wind numbers that arrived at 1:15 and looked the way tax returns look.",
      "The sheet gave wind speed and direction at twenty heights. Every one was within limits. Every launch decision was therefore a yes, and for two years that was fine until a rocket with a very expensive payload left the pad, got halfway up, and politely turned east.",
      "The post-flight review said the winds were fine. Soo-jin did not think they were. The speed at each height had been inside limits, but the direction swung from south at the surface to west at 8 km, with a middle layer pushing hard in between. Each number was innocent. The staircase they made together was not.",
      "She plotted the same twenty rows into a `<Hodograph>` from bui-charts, with `u` and `v` converted from speed and bearing and the height column fed to `height`. The path did not wander like a record of weather. It leapt out from the centre in a long hook, a half circle through the first 6 km, and ended 38 knots from calm at an angle nobody had written down.",
      "Nobody in the room needed shear explained. They needed only to look at the curl in a line and imagine a rocket going up through it. The range officer, a calm man who had said \"looks fine\" at four hundred launches, said nothing for about as long as it takes to read a sentence twice.",
      "They added a rule: the hodograph goes on the launch brief, and any profile that leaves the third ring in a curve gets a second look. Over the next season, three launches were held for that reason, and the one that went ahead a day later landed 220 metres from the target. The sheet of numbers was kept, for the tradition.",
      "Twenty good numbers can add up to one bad wind. The hodograph is the chart that does the adding.",
    ],
  },
  pitfalls: [
    "**Mixing up from and toward.** Meteorological wind is reported as the direction it blows *from*. This chart plots components, so the point shows where the wind is going. Convert before plotting, and say which you used.",
    "**Unequal units.** $u$ and $v$ must be in the same unit. The `unit` prop only changes the axis label, it does not convert.",
    "**Too few levels.** With six rows the line is a few straight segments and any curvature is guesswork. Interpolating the sounding first is better than pretending.",
    "**Colour without a legend.** The colour scale is the only place height lives. If the reader cannot tell which end is the ground, say so in a caption.",
  ],
  related: [
    { slug: "quiver", why: "When you have a vector at many places rather than many heights." },
    { slug: "tephigram", why: "The thermodynamic partner: the same sounding, but with temperature and moisture instead of wind." },
    { slug: "phase-portrait", why: "When the path is a trajectory of a dynamic system rather than a wind profile." },
  ],
  media: [
    {kind:"youtube",id:"4fucgcJeGjQ",title:"How to Read Hodographs - For Beginners",channel:"Storm Chaser Gabe Garfield",note:"A beginner walkthrough of reading wind-shear hodographs for severe-weather forecasting."},
    {kind:"youtube",id:"rVqTX5P1KBo",title:"An Intro to Hodographs",channel:"Tornado Titans",note:"Short intro to what the hodograph curve means and how forecasters use its shape."},
  ],
}
