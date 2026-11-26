# {{title}}

A [BUI](https://bui.preetham.top) app in Ruby, with the `blenderui` gem.

```sh
bui dev            # run it (bundle exec ruby app.rb)
bui doctor         # check the setup and the project, and fix what is wrong
bui update         # move blenderui to the newest version (bundle update blenderui)
```

A Ruby app is not built: there is no `bui build`. The gem builds BUI's C kit while it installs (a few minutes once),
so it needs Ruby 3.1 or newer, Bundler, CMake and a C and C++20 compiler (on Linux the X11 headers too).
Gems are installed into `vendor/bundle` (nothing global). The code is in `app.rb`; it requires `bui`.
