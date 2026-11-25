import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A waveform held still by lining every sweep up on the same rising edge.",
  intro: [
    "A signal that repeats a thousand times a second is, to a human eye, a smear. You can stare at a raw plot of it all afternoon and learn only that it is busy. The oscilloscope's whole trick is not the screen, it is the **trigger**: wait until the voltage rises through a chosen level, start the sweep there, and repeat. Every sweep now begins at the same phase, so the wave stops scrolling and sits still while you look at it.",
    "The instrument grew out of the [cathode ray tube](https://en.wikipedia.org/wiki/Cathode-ray_tube), which [Karl Ferdinand Braun](https://en.wikipedia.org/wiki/Karl_Ferdinand_Braun) built in 1897, and the idea of a stable, triggered sweep is why a bench scope feels like a magnifying glass for time. Nothing about the idea needs a tube any more. If you have samples and timestamps, you can do the triggering in software, which is what this chart does: it finds the first rising crossing of your level and starts the view there.",
    "The result is a line chart with one strong opinion about where the left edge goes. That opinion is the difference between a trace you can measure rise time on and a trace that looks like a seismograph in a bad mood.",
  ],
  math: [
    {
      heading: "Finding the trigger instant",
      body: [
        "Let the samples be $(t_k, v_k)$ for channel 1, the first child you pass, and let $L$ be the trigger level. The chart scans neighbouring pairs from the start of the data and takes the first one that crosses upward: $v_k < L \\le v_{k+1}$. Falling edges and samples that merely touch the level from above do not count.",
        "The crossing almost never lands on a sample, so the instant is found by linear interpolation between the two samples. If $v_{k+1} = v_k$ the fraction is taken as zero instead of dividing by it.",
      ],
      tex: [
        "t^{*} = t_k + (t_{k+1} - t_k)\\,\\frac{L - v_k}{v_{k+1} - v_k}",
      ],
    },
    {
      heading: "From trigger to window",
      body: [
        "The time axis starts at $t^{*}$. With a `span` $S$ the visible window is $[t^{*},\\, t^{*} + S]$ in seconds, so a signal with period $T$ shows $S/T$ cycles. Without a `span` the axis runs to the end of the data. If no sample ever crosses $L$ upward, or you gave no `trigger`, the origin falls back to the first finite timestamp and the trace simply scrolls with your data.",
        "Only channel 1 is used to find $t^{*}$. Every other channel is drawn against the same window. That matches a real scope, where one input is chosen as the trigger source, and it means a second channel with a fixed delay from the first stays put relative to it.",
      ],
    },
    {
      heading: "What else gets drawn",
      body: [
        "The trigger level appears as a thin dashed horizontal line and the instant $t^{*}$ gets a vertical annotation labelled Trigger. Points are never drawn, the line width is 1.5 px, and hovering shows every channel at the nearest sample, because the tooltip is in multi-series mode. Nothing is resampled or smoothed: you see your samples joined by straight segments.",
      ],
    },
  ],
  use: [
    "You have a periodic or repeating signal (a PWM output, a clock, a sensor excitation) and want to see one steady cycle instead of a smear.",
    "You are measuring edges: rise time, overshoot, ringing. These need a stable left edge more than they need anything else.",
    "You want two or three channels compared on the same time base, with channel 1 as the reference everyone else is measured against.",
    "You are replaying captured data from a logic analyser or a DAQ and want the same view you would have had at the bench.",
  ],
  avoid: [
    "The signal is not repeating. A trigger on a one-off transient finds one crossing and tells you nothing about the others. Use [Time Series](/charts/time-series) for the long view.",
    "Your samples are too sparse to define an edge. Interpolating between two points per cycle gives a trigger instant that is a guess with decimals.",
    "You need a spectrum. A trace in time cannot show you which frequencies are present. A [Spectrogram](/charts/spectrogram) can.",
    "Channels have wildly different scales. Put them on separate charts or normalise, or the small one becomes a flat line.",
  ],
  read: [
    "Find the dashed trigger line and the Trigger marker. The wave should cross the line going up exactly at the left edge of the plot.",
    "Count cycles across the window, with a known `span` this gives you the period, and from it the frequency.",
    "Measure what the shape tells you: height for amplitude, the slope of the edge for rise time, wiggles after the edge for ringing.",
    "Hover to read every channel at one instant. Compare channel 2 against channel 1 by how far its edge sits from the left side.",
  ],
  story: {
    title: "The ringing that only happened on Tuesdays",
    paragraphs: [
      "Marek designed power electronics for Lumenwick, a company that made LED drivers for greenhouses. His latest board dimmed its lamps by switching a transistor at 20 kHz, and in the field exactly one customer in forty reported a faint audible buzz. Nobody could reproduce it, which in hardware is the technical term for a ghost.",
      "The lab had a data logger that wrote the gate-drive voltage to a file at 5 MS/s. Marek plotted a slice of it in a normal line chart and got what everyone gets: a dense green block with some texture. He zoomed in and the block turned into an edge, then another edge, then, on the next screen, a different-looking edge. The wave was fine. His view of it kept moving.",
      "What he needed was the oldest trick in the instrument business: pick a voltage, wait for the signal to rise through it, and start every look from there. He needed it in the same tool that held his analysis, on data he already had, without borrowing the one real scope the whole floor shared and guarded like a family heirloom.",
      "He loaded the capture into bui-charts and used `<Oscilloscope>` with two `<TrendSeries>` children, gate drive first and drain voltage second, a trigger at 5 V on the gate and a span of 100 microseconds. Five cycles of clean, motionless wave filled the screen. The dashed line sat at 5 V like a bookmark.",
      "Now the Tuesday problem was visible. In about one cycle in thirty, the drain voltage rang for three extra oscillations after turn-off, because the gate edge was slowing down by a few nanoseconds. That was the difference between the ring decaying inside the dead time and the ring still going when the next pulse arrived. A slow edge is a hard thing to see when the edge keeps moving, and an easy one once it does not.",
      "The cause turned out to be a gate resistor that varied with temperature more than the datasheet admitted. Swapping it for a better part dropped the ringing from 14 V peak to 3 V, and the buzz reports from the greenhouse fell from one in forty to none in the next two hundred units.",
      "The shared scope went back to being guarded. Marek kept his chart, and for the first time anybody could remember, a bug had been caught by waiting for the right moment instead of hoping to be looking at it.",
    ],
  },
  pitfalls: [
    "**Trigger on channel 1.** The first child is the trigger channel. If your reference signal is second in the list, reorder your children.",
    "**A level the signal never crosses.** If no rising crossing exists, the chart does not complain: it falls back to starting at the first sample. A trace that scrolls when you expected it to freeze almost always means the level is above the peak or below the floor.",
    "**Noisy edges.** Noise around the trigger level can cross it several times, and the chart takes the first rising crossing, which may be a noise spike. Choose a level on the steepest part of the edge, away from the noise floor.",
    "**Span in the wrong unit.** Time is in seconds. A 20 kHz wave with a `span` of 100 shows two million cycles. You wanted 0.0001.",
  ],
  related: [
    { slug: "time-series", why: "When the signal is not repeating and you want the whole record rather than one steady cycle." },
    { slug: "spectrogram", why: "When the question is which frequencies are present rather than what one cycle looks like." },
    { slug: "eye-diagram", why: "When you want many unit intervals of a digital link overlaid to judge noise and jitter at a glance." },
  ],
  media: [
    {kind:"youtube",id:"XOv1howMYs8",title:"Precision in under 10 minutes – How to use an oscilloscope",channel:"Rohde & Schwarz",note:"Concise guide from an instrument maker on reading traces and setting timebase and trigger."},
    {kind:"youtube",id:"lSHAE_Y6snc",title:"Oscilloscope Tutorial (Basics 101)",channel:"TheGeekPub",note:"Beginner tutorial covering the screen, controls and common measurements."},
  ],
}
