import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A sounding on a grid of temperature against entropy, where equal areas stand for equal energy.",
  intro: [
    "Every thermodynamic diagram is a compromise. You want isotherms and adiabats to be easy to tell apart, you want pressure to run up the page roughly the way height does, and you want the area on the diagram to mean something. You cannot have all three for free, and that is why meteorology has a small zoo of such charts. Each one gave up a different thing.",
    "The [tephigram](https://en.wikipedia.org/wiki/Tephigram) gives up nothing it cares about. Its name is the plain reading of its axes: T for temperature, the Greek letter phi for entropy, gram for chart. In the early 20th century the British meteorologist [Napier Shaw](https://en.wikipedia.org/wiki/Napier_Shaw) turned this into a working tool, and the Met Office has used it ever since. The two axes are temperature and the logarithm of [potential temperature](https://en.wikipedia.org/wiki/Potential_temperature), which is the entropy of dry air up to a constant, and the whole grid is turned through 45 degrees so that the isobars run nearly horizontal.",
    "The reward is a diagram on which the area of any closed loop is proportional to energy. A parcel of air that you lift through a warmer environment draws such a loop, and the size of the loop is how much energy the storm has to play with. This is exactly the number people want, and the tephigram gives it as a shape you can shade in with a pencil.",
  ],
  math: [
    {
      heading: "Temperature against entropy",
      body: [
        "Potential temperature $\\theta$ is the temperature a parcel would have if moved dry-adiabatically to 1000 hPa. For dry air, specific entropy is a constant times $\\ln\\theta$, so lines of constant $\\theta$ (dry adiabats) are lines of constant entropy. The tephigram plots $T$ against $\\ln\\theta$.",
      ],
      tex: [
        "\\theta = T\\left(\\frac{1000}{p}\\right)^{0.2857}, \\qquad a = T, \\qquad b = 273.15\\,\\ln\\theta",
      ],
    },
    {
      heading: "The 45 degree turn",
      body: [
        "In $(a, b)$ coordinates, isotherms are vertical and dry adiabats are horizontal. The chart rotates the plane by 45 degrees and uses the same scale on both screen axes, so the right angle between the families is kept and the area is preserved up to one constant factor. Here $T$ is in kelvin (the chart converts from Celsius).",
      ],
      tex: [
        "x = \\frac{a + b}{\\sqrt{2}}, \\qquad y = \\frac{b - a}{\\sqrt{2}}",
      ],
    },
    {
      heading: "Why area is energy",
      body: [
        "For a parcel taking a closed path, the work done per unit mass is the integral of $T\\,\\mathrm{d}s$, which is the area enclosed on a diagram of $T$ against entropy $s$. The tephigram plots $s \\propto \\ln\\theta$ against $T$, so the enclosed area is proportional to the energy released. In practice the area between the parcel's path and the environment curve is what forecasters call [CAPE](https://en.wikipedia.org/wiki/Convective_available_potential_energy) where the parcel is warmer, and CIN where it is colder.",
      ],
      tex: [
        "\\oint T\\,\\mathrm{d}s \\;\\propto\\; \\oint T\\,\\mathrm{d}(\\ln\\theta)",
      ],
    },
    {
      heading: "Lines on the grid",
      body: [
        "The background draws isobars every 100 hPa and isotherms every 10 degrees, dry adiabats for $\\theta$ from 220 K to 480 K every 10 K, saturated adiabats integrated from 1000 hPa in 10 hPa steps (the moist lapse rate uses the Bolton saturation vapour pressure and a latent heat of $2.501\\times10^{6}$ J/kg), and mixing-ratio lines from 0.4 to 20 g/kg. The default temperature range across the bottom is $-40$ to $40$ degrees, and the pressure range 1050 down to 100 hPa. Levels are sorted by pressure before drawing, and levels with a non-positive or non-finite pressure are dropped.",
      ],
    },
  ],
  use: [
    "You have a **sounding** and want to estimate convective energy by area, as CAPE and CIN.",
    "You work in the UK or Commonwealth, or a field where the tephigram is the standard.",
    "You need a diagram where an enclosed area has a physical meaning, for teaching or for a quick estimate.",
    "You want one grid showing dry and moist adiabats and mixing ratio together, to find cloud base and the level of free convection.",
  ],
  avoid: [
    "Your audience is American and reads Skew-T log-P charts. A tephigram will cost you five minutes of explanation before it saves any.",
    "You want a grid on which straight lines are easy to draw by hand. The [Stüve](/charts/stuve) has straight dry adiabats.",
    "You want wind. Draw a hodograph next to it.",
    "You want a plain temperature-height plot for a general audience. A line chart does that better.",
  ],
  read: [
    "Find the red temperature line and the green dew point. A small gap means nearly saturated air.",
    "Follow the dry adiabat up from the surface temperature, and the mixing-ratio line up from the surface dew point. Where they cross is the lifting condensation level, roughly the cloud base.",
    "From there, follow the moist adiabat upward. Wherever it lies to the warm side of the red line the parcel is buoyant, and the area between them is the energy available.",
    "Hover a level for its temperature, dew point, mixing ratio and potential temperature.",
  ],
  story: {
    title: "The area under the afternoon",
    paragraphs: [
      "Marguerite taught atmospheric physics to second-year students and had a tradition that she regretted every year around week six. The tradition was the thermodynamics lecture, in which she would derive CAPE from the first law, fill a whiteboard, turn around and find that 140 undergraduates had also turned into whiteboards.",
      "The problem was real. Convective available potential energy is an integral, and an integral is an area, and an area is a thing you can see. Nobody had ever shown her students the area. They had seen the formula, the table of constants and a slide with a stock photo of a thunderstorm, which was supposed to help.",
      "That year she decided to do it differently. She took a sounding from a day when a storm had formed over the campus at 4 p.m. and given the football team an early shower. The 12 UTC balloon had been launched from an airfield 20 km away, with 94 levels of pressure, temperature and dew point.",
      "She plotted it in a bui-charts `<Tephigram>`, handing the three columns to `pressure`, `temperature` and `dewpoint`. Then she had the class lift a parcel from the surface by hand: up the dry adiabat, across to the mixing-ratio line, then along the moist adiabat, which was already drawn on the grid.",
      "Between 780 and 340 hPa the parcel's path leaned to the warm side of the red line, and the sliver between them was about the size of a thumbnail. She had the students shade it with a highlighter on a printout. It was CAPE. It was also, as one of them noted, \"smaller than I expected for something that ruined football.\" She told him that this was a perfectly good question, and that the answer was 1,900 joules per kilogram, which is more than enough.",
      "In the next test, 82% of the class could identify the level of free convection correctly, against 51% the year before. Marguerite credits the highlighter. The department credits the tephigram. The football team credits no one and still does not trust balloons.",
      "A formula says how much. A shaded area says why you should care.",
    ],
  },
  pitfalls: [
    "**Reading it like a Stüve.** The isotherms and adiabats are at 45 degrees to the page, not vertical and straight. Follow the labelled lines, not your instinct.",
    "**Using it outside its area.** The default temperature range is $-40$ to $40$. A sounding that drifts outside simply leaves the frame. Widen `temperature_range` for polar or stratospheric work.",
    "**Dew point errors high up.** Humidity sensors above 400 hPa are unreliable. Do not read much from a dew point there.",
    "**Treating the moist adiabats as exact.** They are integrated from 1000 hPa with a simple midpoint scheme, which is plenty for a forecast sketch and not a substitute for a parcel model in a research paper.",
  ],
  related: [
    { slug: "stuve", why: "The same sounding on a grid with straight dry adiabats, easier to draw by hand." },
    { slug: "hodograph", why: "The companion for wind shear, which together with CAPE decides how severe a storm gets." },
    { slug: "ternary-contour", why: "Another chart where a physical field is drawn over a fixed grid." },
  ],
  media: [
    { kind: "youtube", id: "7p7c85hhgOo", title: "Visual Explanation of Meteorological Skew T Log P Sounding Diagrams", channel: "Patrick Brown", note: "Not a tephigram, but the same family: it builds up isobars, isotherms, adiabats and mixing-ratio lines step by step." },
  ],
}
