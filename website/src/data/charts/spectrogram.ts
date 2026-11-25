import type { ChartDoc } from "./types"

export const doc: ChartDoc = {
  tagline: "A signal's frequencies over time: time across, frequency up, power as colour.",
  intro: [
    "A waveform tells you how loud something was and when. It tells you almost nothing about what it was. A flute and a trombone playing the same note at the same volume draw similar-looking squiggles, and a human can tell them apart in a tenth of a second. The information is in the frequencies, and a [spectrogram](https://en.wikipedia.org/wiki/Spectrogram) is the picture of those.",
    "Spectrograms have been around since the 1940s, when Bell Labs built a machine that turned speech into a visible pattern, and a book titled *Visible Speech* was published on the premise that people could be trained to read it. (Few were. It turns out that listening is quicker.) Today the same picture is the front end of speech recognition, bird-song identification, vibration analysis in machines, and the sonar of almost anything.",
    "The reading is straightforward once you have seen a few. A steady note is a horizontal line. A siren is a diagonal. A drum hit is a vertical smear from bottom to top, because a click contains every frequency at once. The difficulty is not the reading but the one setting that changes what you see: the window.",
  ],
  math: [
    {
      heading: "The short-time Fourier transform",
      body: [
        "The signal is $x_n$, sampled at a steady rate. The chart reads the rate from your `x` column: it takes the median of the positive gaps between consecutive times, calls that the sample step $\\delta$, and sets $f_s = 1/\\delta$. Pass time in seconds and the frequency axis is in Hz. Non-finite samples count as zero.",
        "The signal is cut into windows of $N$ samples (`window_size`, default 256, rounded up to a power of two, at least 8). Each window is multiplied by a [Hann taper](https://en.wikipedia.org/wiki/Hann_function), which fades its edges to zero so that the cut does not leak energy across the spectrum, and then transformed with a fast Fourier transform.",
      ],
      tex: [
        "w_n = \\tfrac12 - \\tfrac12\\cos\\!\\Bigl(\\frac{2\\pi n}{N-1}\\Bigr), \\qquad X_k = \\sum_{n=0}^{N-1} w_n\\, x_{s+n}\\, e^{-2\\pi i k n / N}",
      ],
    },
    {
      heading: "Power in decibels",
      body: [
        "Each window gives the bins $k = 0, \\dots, N/2 - 1$, the lower half of the spectrum (the upper half is a mirror image for a real signal, and the top bin is the Nyquist limit $f_s/2$). Bin $k$ is centered on $f_k = k f_s / N$ and spans one bin width $f_s / N$.",
        "The cell's colour is the power in decibels. A tiny constant keeps the logarithm finite where the power is zero.",
      ],
      tex: [
        "P_k = \\frac{|X_k|^2}{N}, \\qquad L_k = 10\\,\\log_{10}\\!\\bigl(P_k + 10^{-12}\\bigr), \\qquad f_k = \\frac{k\\, f_s}{N}",
      ],
    },
    {
      heading: "The window trade-off and the hop",
      body: [
        "A window of $N$ samples lasts $N/f_s$ seconds and separates frequencies that are about $f_s/N$ Hz apart, so one number sets both resolutions: a long window sees close pitches clearly and smears events in time, and a short window pins the moment and blurs the pitch. You cannot have both, which is the Heisenberg-style uncertainty of signal analysis.",
        "Windows advance by a hop of $\\lfloor N(1 - \\text{overlap})\\rfloor$ samples (`overlap` is clamped to 0 to 0.95, default 0.5, and the hop is at least one sample). Only full windows are used, so a tail shorter than $N$ samples is dropped. Each cell is drawn as wide as one hop, starting at the window's first sample. Overlap gives smoother columns without changing the trade-off.",
      ],
      tex: ["\\Delta f = \\frac{f_s}{N}, \\qquad \\Delta t = \\frac{N}{f_s}, \\qquad \\Delta f \\cdot \\Delta t = 1"],
    },
  ],
  use: [
    "You have a signal sampled at a steady rate and the question is *which frequencies, when*: audio, vibration, EEG, radio, a heartbeat, a turbine.",
    "You are looking for events that are hard to see in the waveform: a bearing starting to whine, a bird call under traffic noise.",
    "You want to see a trend in the frequency content: pitch gliding, a harmonic fading, a hum that comes and goes.",
    "You need to compare two recordings by structure rather than by loudness.",
  ],
  avoid: [
    "The signal is not evenly sampled. The chart takes the median step and assumes it holds. Resample first or the frequency axis is fiction.",
    "You only need the overall spectrum. A single spectrum, not a function of time, is clearer as a [line](/charts/trend) of power against frequency.",
    "You care about phase. A spectrogram keeps the magnitude and throws the phase away.",
    "The signal is very short (fewer samples than the window). There are no full windows, so nothing is drawn.",
  ],
  read: [
    "Time runs left to right in seconds, frequency runs bottom to top in Hz, and the top of the axis is half the sample rate.",
    "The default colour scheme is magma: dark is quiet, bright is loud. Values are in dB, so a step of 10 is ten times the power.",
    "A horizontal line is a steady tone. A stack of evenly spaced lines is a note with its harmonics.",
    "A diagonal is a sweep (rising or falling pitch). A vertical stripe is a click or an onset.",
    "Hover a cell to read its time, frequency and level in dB.",
  ],
  story: {
    title: "The hum at 2.4 kilohertz",
    paragraphs: [
      "Amira was a reliability engineer at a small company that made industrial fans, and she had a customer who was convinced that one of the fans \"sounded different on Thursdays\". The customer had said this on three calls. Amira had the manner of a person who has decided to believe a statement only when it comes with data.",
      "The fan had a vibration sensor sampling at 10 kHz, and it had been logging for weeks. She pulled a 30-second capture from a Wednesday and one from a Thursday and plotted both waveforms. They looked identical: a fuzzy band of noise of the same height. The customer was either delusional or hearing something the waveform could not show.",
      "What the waveform could not show was frequency. She needed a picture in which a fault that lived at one specific pitch would show as one specific line, and in which a Thursday-only behaviour would show as a line that started partway through.",
      "She loaded the Thursday capture into a `<Spectrogram>` from bui-charts with a window of 1024 samples, so that each bin was about 10 Hz wide, and left everything else alone. Time and value, two accessors, and a magma map she did not need to choose.",
      "It was there at once: a thin bright line at 2.4 kHz that began about eight seconds in and stayed. Wednesday's capture had no such line. A bright line that starts in the middle of a recording is a mechanical event, and the shaft that drove the fan had a single bearing at that rotation rate.",
      "They replaced the bearing the following week. The line was gone, and the average vibration energy in the 2 to 3 kHz band dropped by about 14 dB. The customer said it sounded the same, which Amira took as the best possible review.",
      "She keeps a screenshot of the line on her desktop. She calls it the Thursday, which is not an exact translation of what it is but has stuck.",
    ],
  },
  pitfalls: [
    "**Window size is a choice, not a default to ignore.** A 256-sample window at 8 kHz resolves about 31 Hz and lasts 32 ms. If your tones are closer than that, they will fuse into one, if your events are shorter, they will smear. Try both before you conclude anything.",
    "**Wrong time units.** The frequency axis is only in Hz if your `x` is in seconds. Pass milliseconds and every frequency is 1000 times too small.",
    "**[Aliasing](https://en.wikipedia.org/wiki/Aliasing).** Anything above half the sample rate folds back into the picture as a lower frequency. The spectrogram cannot tell you it did. Filter before sampling.",
    "**Reading dB as linear.** A cell that looks twice as bright is not twice the energy. Ten decibels is a factor of ten in power, so small colour changes can be large physical ones.",
  ],
  related: [
    { slug: "heatmap", why: "When your grid is already time against value buckets and you do not need a Fourier transform." },
    { slug: "oscilloscope", why: "When you want the waveform itself in time, not its frequencies." },
    { slug: "eye-diagram", why: "When the signal is a high-speed digital link and you want to see its timing margin." },
  ],
  media: [
    { kind: "youtube", id: "0loVfl2Lq4s", title: "Spectrogram explained", channel: "Srinath Srinivasan", note: "A short, direct explanation of axes and colour, enough to read your first spectrogram." },
  ],
}
