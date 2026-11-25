import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A bit stream folded onto itself so thousands of transitions can be judged at once by how open the gap in the middle is.",
  intro: [
    "A serial link sends ones and zeros as voltage, and the receiver has to decide, in a window a fraction of a nanosecond wide, which one it just saw. Whether it gets it right depends on three things: how high the high is, how low the low is, and how consistently each transition happens at the same time. A plain waveform shows one of those at a time and a million of them as a green smear.",
    "The [eye diagram](https://en.wikipedia.org/wiki/Eye_pattern) is the smear, made useful on purpose. You cut the signal into pieces one or two bit periods long and draw them all on top of each other. Wherever bits behave the same way, the traces pile up into thick lines. Wherever they differ, the traces spread out. In the middle is a gap where no trace passes, and that gap is shaped like an eye. Wide and open: good link. Squinting and half closed: go find the cable.",
    "Oscilloscopes have drawn these since the days of cathode-ray tubes, which is where the persistence phosphor did the stacking for free. You can build the same picture from any sampled signal in software, and that is what this chart does. It also counts where the signal crosses the middle level and shows those crossings underneath, because the *width* of that crowd is the timing [jitter](https://en.wikipedia.org/wiki/Jitter), and jitter is what actually kills links.",
  ],
  math: [
    {
      heading: "Folding time into a window",
      body: [
        "Each sample is a pair $(t_i, v_i)$. Choose a unit interval $T$ (the `period`), a number of unit intervals $n$ to display (`spans`, 2 by default) and an `offset` $t_0$ that lands on the left edge. The window is $W = nT$. A sample is drawn at its phase inside that window, and every trace that started in a different window lands on the same axis.",
      ],
      tex: [
        "W = n\\,T, \\qquad \\varphi_i = (t_i - t_0) \\bmod W \\;\\in\\; [0,\\, W)",
      ],
    },
    {
      heading: "Keeping each trace continuous",
      body: [
        "If you just plotted $(\\varphi_i, v_i)$ in order, the line would draw a horizontal streak from the right edge back to the left every time the phase wrapped. The chart instead detects the wrap (the phase drops by more than half a window), finds the value where the signal would have crossed the window edge by linear interpolation, ends the trace at the right edge, and starts the next one at the left edge from the same value. The traces are drawn at a low opacity (0.25 unless you change `trace_opacity`) so overlaps add up into brightness.",
      ],
      tex: [
        "v_{\\text{edge}} = v_{k} + \\frac{W - \\varphi_k}{t_{k+1} - t_k}\\,(v_{k+1} - v_k)",
      ],
    },
    {
      heading: "The crossing histogram",
      body: [
        "The strip underneath counts threshold crossings. The threshold $L$ defaults to halfway between the lowest and highest value in the data. A crossing happens between samples $k$ and $k+1$ whenever the two sit on opposite sides of $L$, and its time is found by interpolation. Its phase is binned into `bins` equal slots (60 by default) across the window.",
      ],
      tex: [
        "(v_k - L)(v_{k+1} - L) < 0 \\;\\Rightarrow\\; t^{\\ast} = t_k + \\frac{L - v_k}{v_{k+1} - v_k}\\,(t_{k+1} - t_k)",
      ],
    },
  ],
  use: [
    "You are judging **signal integrity** on a serial link, a memory bus, a clock or any NRZ-like signal.",
    "You want a single picture of *thousands* of bit transitions, so one rogue edge does not hide among a million normal ones.",
    "You want to see timing jitter and voltage noise separately: horizontal spread at the crossing and vertical thickness of the rails.",
    "You are comparing two cables, two fixtures or two equalizer settings on the same data pattern.",
  ],
  avoid: [
    "The signal is not periodic in a known interval. Without a correct `period` the traces fold at the wrong width and the eye smears.",
    "You need to see a *specific* rare event, such as a single glitch at 3.2 ms. A folded plot erases when things happened.",
    "You have only a few hundred samples. An eye is a statistical picture, and a few traces draw a few lines, not an eye.",
    "You want a pass or fail result with a mask. This chart shows the picture. It does not test it against a standard mask.",
  ],
  read: [
    "Look at the opening in the middle. Height is voltage margin, and width is timing margin. Together they are the room the receiver has to decide.",
    "Look at how thick the two rails are. A fuzzy top and bottom are amplitude noise, and a clean eye with ragged crossings is a timing problem.",
    "Look at the crossing strip underneath. A narrow spike is good. A wide, lumpy hill means jitter, and two humps mean the edges depend on the data pattern.",
    "Ctrl+scroll zooms the eye and the strip together, so you can inspect the crossing without losing the window.",
  ],
  story: {
    title: "The cable that only failed on Tuesdays",
    paragraphs: [
      "Amara ran the test lab at a company that made industrial cameras, and the company's difficulty was a cable. The camera sat on a robot arm and sent images to a controller 15 metres away over a cable that met every datasheet and still dropped frames, but only on Tuesdays, which seemed implausible even for hardware.",
      "The first theory was software. The second was the robot's motor. The third, advanced by a salesman, was that Tuesday was the day the factory ran the large press on the next floor. None of these was wrong, and none of them was a measurement.",
      "She captured four million samples of the link at the controller input during a bad stretch, which is a great way to produce a file that no one can read. A plain waveform at that length is a thick bar of colour. She needed a way to ask a single question of the file: is the receiver being given enough room to decide?",
      "She loaded the samples into a bui-charts `<EyeDiagram>`, with the bit period of 400 picoseconds as `period` and the default two-bit window. On a good day the eye was a tidy diamond about 0.55 of a bit wide. On the bad capture it had closed to a slit, and the crossing strip below was no longer a spike but a broad double hill about 120 ps wide.",
      "Two humps meant the edge timing depended on the data. That is the signature of inter-symbol interference, which means the cable was low-passing the bit stream, with long runs of identical bits ending up with different amplitudes than short ones. The press upstairs had nothing to do with it. A flexed connector on the robot arm had changed the cable's loss, and Tuesday was when the arm ran its full-reach routine.",
      "She swapped the connector for a better one and added a 3 dB boost to the transmitter. The eye reopened to 0.62 and the dropped frames went from about 40 a day to none in the following month. The press was officially cleared of all charges.",
      "Every link has a bad day. The eye diagram is the only chart that lets you see exactly how much room it left you.",
    ],
  },
  pitfalls: [
    "**Wrong `period`.** If it is off by even a percent, the traces slowly slide across the window and the eye blurs into a band. Take the period from the data rate, not from eyeballing the waveform.",
    "**Offset not at the bit centre.** Choose `offset` so the eye is in the middle of the window, otherwise the crossings sit on the edge and the picture is hard to read.",
    "**Sampling too coarse.** With fewer than about 8 samples per period the interpolation of edges is crude, and the crossing histogram gets lumpy.",
    "**Threshold at the wrong level.** The default is halfway between the extremes. A signal with overshoot or a DC offset can shift the true decision level, so set `threshold` yourself in those cases.",
  ],
  related: [
    { slug: "oscilloscope", why: "When you want to see the signal in time order rather than folded." },
    { slug: "spectrogram", why: "When the problem is which frequencies are in the signal rather than how the edges line up." },
    { slug: "smith-chart", why: "The other classic chart for high-frequency links, for how well a load matches the line." },
  ],
  media: [
    { kind: "youtube", id: "MkTbNov6geQ", title: "The Eye Diagram: Visualizing Signal Integrity in High Speed Links", channel: "Learning By Tutorials", note: "A walk-through of what the eye height, width and jitter mean and how they relate to link margin." },
  ],
}
