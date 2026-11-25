import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A weather balloon's temperature and dew point against pressure, on a grid where dry air rises along straight lines.",
  intro: [
    "A weather balloon climbs through the atmosphere sending back pressure, temperature and dew point every few seconds. That column of numbers is called a sounding, and it answers a question that matters more than most people expect: if you pushed a parcel of air upward, would it keep going or fall back? If it keeps going you get clouds, then storms. If it falls back you get a pleasant afternoon and a pilot who can see.",
    "To answer, you need to compare the sounding with what a parcel does as it rises. A parcel expands as the pressure drops, cools at a steady rate if it is dry, and cools more slowly once it starts to condense. These paths are called [adiabats](https://en.wikipedia.org/wiki/Lapse_rate), and a thermodynamic diagram draws them in advance as a background grid, so the forecaster only has to lay the sounding on top and look.",
    "The Stüve diagram is the simple version of this. It is named for the German meteorologist Georg Stüve, who proposed it in the 1920s. It plots temperature across and pressure raised to the power 0.286 upward, and for that one clever choice, dry adiabats come out as straight lines. You can draw one with a pencil and a ruler, which for a century has been a selling point in places where the computer is at lunch.",
  ],
  math: [
    {
      heading: "Why pressure to the 0.286",
      body: [
        "A parcel of dry air that rises or sinks without exchanging heat keeps its [potential temperature](https://en.wikipedia.org/wiki/Potential_temperature) $\\theta$, the temperature it would have if brought down to 1000 hPa. The relation, with $T$ in kelvin and $p$ in hPa, uses $\\kappa = R/c_p \\approx 0.2857$.",
      ],
      tex: [
        "\\theta = T\\left(\\frac{1000}{p}\\right)^{\\kappa}, \\qquad \\kappa = 0.2857",
      ],
    },
    {
      heading: "The coordinates",
      body: [
        "On a dry adiabat $\\theta$ is constant, so $T = \\theta\\,(p/1000)^{\\kappa}$. If the vertical coordinate is $y = p^{\\kappa}$, this is a straight line through the origin of $(T, y)$ with slope proportional to $\\theta$. The chart draws pressure upward with the smaller values on top, so the vertical coordinate is $-p^{\\kappa}$. The horizontal axis is temperature in Celsius, from $-80$ to $40$ by default, and the vertical axis runs from `pressure_range`, 1050 hPa at the bottom to 100 hPa at the top by default.",
      ],
      tex: [
        "x = T_{\\mathrm{C}}, \\qquad y = -p^{0.2857}",
      ],
    },
    {
      heading: "Moist adiabats and mixing ratio lines",
      body: [
        "Saturation vapour pressure uses the Bolton formula, and the saturation mixing ratio $w_s$ follows, with $\\varepsilon = 0.622$. Mixing-ratio lines (0.4 to 20 g/kg) are drawn by inverting this for the dew point at each pressure.",
      ],
      tex: [
        "e_s(T) = 6.112\\,\\exp\\!\\left(\\frac{17.67\\,T}{T + 243.5}\\right), \\qquad w_s = \\varepsilon\\,\\frac{e_s}{p - e_s}",
      ],
    },
    {
      heading: "What is drawn",
      body: [
        "Dry adiabats are drawn for $\\theta$ from 220 K to 480 K every 10 K. Saturated adiabats start at 1000 hPa at temperatures from $-20$ to $40$ degrees Celsius in steps of 4 and are integrated numerically in 10 hPa steps with a midpoint method, using the latent heat of vapourisation and the specific heat of dry air. Isobars are every 100 hPa from 1000 to 100, and isotherms every 10 degrees. The temperature profile is red, the dew point green. Levels are sorted by pressure, so you can pass them in any order.",
      ],
    },
  ],
  use: [
    "You have a **sounding** and want to judge stability, cloud base and the chance of convection.",
    "You want to draw by hand, or teach someone how thermodynamic diagrams work. Dry adiabats are straight, so the rules fit on a napkin.",
    "You want to compare the environment with a lifted parcel and see where it is warmer, which is where air will rise.",
    "You want the isobars close to horizontal and the temperature scale wide enough for the whole troposphere and some of the stratosphere.",
  ],
  avoid: [
    "You need to compare **areas as energy**. This chart's area has no tidy meaning, for that, use the [tephigram](/charts/tephigram), where equal areas are equal energy.",
    "You want winds. A sounding diagram carries temperature and moisture only. Add a [hodograph](/charts/hodograph) beside it.",
    "Your data is a time series at one place. Use a normal line chart.",
    "Your audience reads Skew-T log-P charts, the American standard, and gets suspicious of anything else. A Stüve is a fine chart, but not the one they were taught.",
  ],
  read: [
    "Find the red temperature line and the green dew point. Where they are close together, the air is nearly saturated and cloud is likely at that level.",
    "Compare the red line with the nearest dry adiabat. A layer where the temperature drops with height faster than the adiabat is unstable. A layer that cools more slowly, or warms (an inversion), is stable.",
    "Follow a dry adiabat up from the ground until it meets a mixing-ratio line through the surface dew point: that is where cloud base sits.",
    "Hover a level to read its temperature, dew point, mixing ratio and potential temperature.",
  ],
  story: {
    title: "The Thursday that never fell apart",
    paragraphs: [
      "Gustavo flew gliders on weekends and, to the great annoyance of his family, spent most of the week looking at weather. The club ran a Thursday evening briefing, and for years the same ritual happened. Someone read out a forecast, someone else said \"I don't trust it,\" and everyone went home with the feeling of a decision having been made.",
      "He wanted something better, and the club had one thing going for it: a free sounding came in every morning from a station 60 kilometres away. It was a long text table of pressure, temperature and dew point, 70 rows of it, which gave every member the same view as a bus timetable.",
      "He loaded the table into a bui-charts `<Stuve>`, and fed the three columns to `pressure`, `temperature` and `dewpoint`. Instantly the table became a picture with an obvious story. The red line leaned nicely along the dry adiabats up to about 700 hPa, so the lower air was well mixed. Above it was a warm layer, an inversion with a lid shaped like a kink in the line.",
      "A lid means that thermals climb and then stop. In practice, that is the difference between a day of cloud-base at 3000 metres and a day when you circle in a bubble and drift into someone's field. The green dew point line crossed the dry adiabat from the surface at 840 hPa. That was the cloud base, about 1600 metres above the ground, and the lid sat only 400 metres above it.",
      "He printed it, taped it to the clubhouse window, and wrote on it with a marker: \"Cumulus, flat, no climb above 2000.\" Three members disagreed, because it was Thursday and that is what the day is for. Saturday agreed with the chart. Cloud base was 1650, the cumulus topped out at 2050 metres, and the one pilot who tried for a 100 km flight landed in a stubble field 61 km out.",
      "The club now looks at the chart before the forecast. One member, who had flown for thirty years and trusted his knee more than the Met Office, admitted that the picture was good. He did not say that out loud. He asked for a copy.",
      "A sounding is a table until someone draws it on the right grid. Then it is a forecast with a straight edge.",
    ],
  },
  pitfalls: [
    "**Temperature in the wrong unit.** Both temperature and dew point are in degrees Celsius, and pressure in hPa. Kelvin or Pa will put the whole profile off the chart.",
    "**Dew point above temperature.** Not physical. If a sensor says so, it is noise or a wet sensor, and the profile will cross itself in a way that looks meaningful but is not.",
    "**Narrow ranges.** `pressure_range` and `temperature_range` crop the grid. A tight window shows more detail of the boundary layer, but the adiabats will leave the picture and some labels will vanish.",
    "**Wrong mental model of the grid.** Pressure is spaced as $p^{0.286}$, not linearly, so the layers are not equal in height. Thickness is not a vertical distance on this chart.",
  ],
  related: [
    { slug: "tephigram", why: "The same sounding on a grid where area is energy, preferred in the UK and Commonwealth." },
    { slug: "hodograph", why: "The wind half of a sounding: how the wind changes with height." },
    { slug: "ternary-contour", why: "Another chart that draws a physical field over a fixed grid." },
  ],
  media: [
    { kind: "youtube", id: "mYJw2QovUKQ", title: "Skew-T Diagram Basics", channel: "NOAA Weather Partners", note: "Covers the same ingredients (isobars, isotherms, adiabats, mixing ratio) on the Skew-T that the Stüve draws with straight adiabats." },
  ],
}
