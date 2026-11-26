# {{title}}

A [BUI](https://bui.preetham.top) app in Go, with the module in BUI's `bindings/go` (views in MyGo's style).

```sh
bui dev            # run it (go run .)
bui build          # build it (go build)
bui doctor         # check the toolchain and the project, and fix what is wrong
bui update         # move the BUI checkout to the newest release and rebuild its kit
```

The Go module cannot be fetched on its own: it needs BUI's C sources next to it. `bui` keeps one shallow checkout
of the newest release in `~/.bui/BUI`, shared by every Go app, and `go.mod` points at it with a `replace` line
(an absolute path on this machine: change it if the checkout lives somewhere else, or if you share the project).
`go generate github.com/ppmpreetham/BUI/bindings/go/bui` builds the C kit into that checkout's `.bui/` folder.

Go 1.22 or newer with cgo needs a C compiler: gcc or clang (MinGW-w64 gcc on Windows), CMake, and on Linux the X11 headers.
