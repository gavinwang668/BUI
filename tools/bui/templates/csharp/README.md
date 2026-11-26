# {{title}}

A [BUI](https://bui.preetham.top) app in C#, with the `BlenderUI` NuGet package on .NET 8.

```sh
bui dev            # run it (dotnet run)
bui build          # a release build (dotnet build -c Release)
bui doctor         # check the setup and the project, and fix what is wrong
bui update         # move BlenderUI to the newest version
```

The package carries BUI's kit for Linux x64, macOS (arm64 and x64) and Windows x64, so there is nothing to compile.
The code is in `Program.cs`. The app rolls forward to a newer .NET runtime when .NET 8 is not installed.
