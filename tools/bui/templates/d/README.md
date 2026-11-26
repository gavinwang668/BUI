# {{title}}

A [BUI](https://bui.preetham.top) app in D, with the `blenderui` DUB package.

```sh
bui dev            # run it (dub run, with LDC)
bui build          # a release build (dub build -b release)
bui doctor         # check the toolchain and the project, and fix what is wrong
bui update         # move blenderui to the newest version
```

The first build compiles BUI's C kit with CMake into the package's `.bui/` folder (a few minutes once, seconds after).
The code is in `source/app.d`; `import bui;` gives you the nodes, contexts, windows and the HTML, CSS and RSX modules.
