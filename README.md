# VI Causeway

VI Causeway is an experimental Windows compatibility layer for the Xbox Series X|S version of Grand Theft Auto VI. It runs the game's x86-64 code on the host CPU, implements the Xbox services the game calls, and translates its graphics commands and shaders for Direct3D 12. You supply your own game files. It builds on the work of Marvisak and the VIctory contributors. [Upstream project](https://github.com/Marvisak/VIctory)

## Startup status

On Windows 10 22H2 with an Intel Core i5-11600K and a GeForce RTX 3060 Ti, we verified the first-boot sequence for Xbox build `1.0.2.1`. Selecting the profile on the game's opening screen played the Rockstar Games animation with sound, then brought up the first-time settings and legal screens. The game saved the profile and settings locally and began loading the story. On a first-boot test the title's stall watchdog terminated the process during the first world frames. On a returning profile, the opening led directly to the illustrated story loading screens. A Menu button press reached the input layer, but the pause menu did not render during the test.

In a release-build comparison on that machine, the opening movie ran at 30 fps. Reusing texture row spans reduced reported texture preparation from 14.8–15.7 to 3.1–3.4 ms per movie frame. A process-only audio capture matched the original soundtrack through its fade with no measurable accumulated delay.

On a fresh profile, choose **Settings** at **Finish Setup?** with the controller Menu button or the **Esc** key. The game opens its seven-tab Settings menu before starting the story. We opened Control Scheme, Controller, Gameplay, and Display and used Back to return to setup.

On this Windows 10 and NVIDIA system, a separate Series S trial presented black frames through the player-selection phase. The verified startup above used the Series X profile.

Upstream VIctory separately reports reaching the first mission on Windows 11 with a Radeon RX 6700 XT. Its world frames were black and took 1.5–3 seconds to render. [Upstream test results](https://github.com/Marvisak/VIctory/blob/1dd7afd4e68300e5dea41cd33a6e0b297798bb2f/README.md#status-2026-11-23)

Upstream reports that entering the world can reset the Windows display driver. The offline profile, opening movie, and first-run screens can be tested before entering the world.

## Building on Windows 10

Use the x64 MSVC Rust toolchain pinned by this workspace, the Visual Studio C++ Build Tools and a Windows SDK. From the repository root, build the release executable with the Agility feature. For frame capture and other diagnostics, build the diagnostic profile with both features.

```powershell
cargo build --release -p victory --features agility
cargo build --profile diag -p victory --features diagnostics,agility
```

Download Microsoft's `Microsoft.Direct3D.D3D12` NuGet package version `1.620.2` and open the package as a ZIP archive. Copy `build/native/bin/x64/D3D12Core.dll` into a `D3D12` subdirectory beside each executable you plan to run, such as `target\release\D3D12` or `target\diag\D3D12`. If you enable the Direct3D debug layer, copy the matching `d3d12SDKLayers.dll` into the same directory. [Microsoft Agility SDK package](https://www.nuget.org/packages/Microsoft.Direct3D.D3D12/1.620.2)

## Credits and license

VI Causeway is an independent derivative of VIctory by Marvisak and the VIctory contributors. This project began on 2026-11-23. The starting revision was `1dd7afd`. The imported history retains the original authors and co-author credits. Commit IDs changed where the history was filtered to omit the upstream BCPack table. The compatibility layer is licensed under GPL-3.0-or-later. [Original upstream revision](https://github.com/Marvisak/VIctory/tree/1dd7afd4e68300e5dea41cd33a6e0b297798bb2f) · [License](LICENSE)

Two GPU fixes helped the startup sequence render on NVIDIA hardware. Nostyxx remapped descriptor tables that exceed the host heap limit. Ryx (Ryx69) kept shader resource views stable until the command list resets. [Nostyxx's heap fix](https://github.com/Marvisak/VIctory/pull/21) · [Ryx69's descriptor fix](https://github.com/Marvisak/VIctory/pull/20)

The maintainer guide describes how this project imports later changes. [Updating from VIctory](docs/upstream.md)

## Getting started

Requirements: Windows 10/11 x64, a CPU with AVX2, a Direct3D 12 GPU with
shader model 6.6 (and mesh shaders for the world), Rust (pinned in
`rust-toolchain.toml`).

```powershell
cargo xtask install-hooks                # asset-policy check on every commit
copy victory.example.toml victory.toml   # then set your paths (outside the repo)
cargo test --workspace                   # needs no game data
cargo run -p victory -- config           # check your paths
```

For a title that reads BCPack-compressed textures, set `[paths].bcpack_tables`
in `victory.toml` to your complete BCPack decoder table file outside the
repository. You can override that path with `VICTORY_BCPACK_TABLES`.
The file is 77,982 bytes and follows the layout documented by
`victory_formats::bcpack::Tables::parse`. Builds and synthetic tests run
without this file.

Run the title with the executable you built. The executable and configuration retain their upstream names for compatibility.

```powershell
.\target\release\victory.exe run "D:\games\my-dump\game_scarlett_master.exe" --execute
```

For a 60 Hz presentation target, append `--fps 60` to the `run` command. Omit the flag for the title's native timing, or use `--fps 30` to select 30 Hz. The option changes the timing of the console's standard 30/60 Hz frame requests for this launch. It preserves the save settings.

On the i5-11600K and RTX 3060 Ti, the title submitted up to 60 frames per second on the player screen and during the opening movie. The video retains its original 30 fps content and the audio played at its original speed. The illustrated loading screen still presented at 30 fps. The world currently runs below either target.

The window caption shows the framerate. Saves, settings and `crashes.log`
are kept under the data folder (default `%LOCALAPPDATA%\victory\data`), derived
data under the cache folder.

XInput controllers are supported. With none connected the keyboard is a pad while the
title's window has focus:

| Keys | Pad |
|------|-----|
| W A S D / I J K L | left / right stick |
| arrows | D-pad |
| Enter, Backspace, X, Y | A, B, X, Y |
| Q / E, Z / C | shoulders, triggers |
| F / H | stick clicks |
| Esc, Tab | Menu, View |

## Commands

```
victory inspect <exe|dll> [--imports] [--exports]
victory census [paths...] [--missing] [--strict]
victory lib-exports <import.lib> [--list]
victory shader-info [paths...] [--verbose] [--disasm]
victory translate [paths...] [--validate] [--pipelines] [--list] [--jobs N]
victory run <exe> [--execute] [--unimplemented trap|zero] [--strict]
            [--console series-x|series-s] [--fps 30|60] [-- <title arguments>]
victory config
```

Logging: `VICTORY_LOG=victory_loader=trace,info` (a release build logs
info and above).

## Debugging

A release build has no debugging aids. Build them in with

```powershell
cargo build --profile diag -p victory --features diagnostics   # target\diag\victory.exe
```

| Variable | Effect |
|----------|--------|
| `VICTORY_LOG=<target>=debug,...` | debug and trace output |
| `VICTORY_STACKS=20,60` | every thread's stack at those seconds |
| `VICTORY_WATCH=<rva>,...` | logs a title u32 when it changes |
| `VICTORY_KEYS=30:Enter,35:X` | presses keys at those seconds, focus or not |
| `VICTORY_CRASH_SAVE=<rva>:<len>,...` | saves title memory to the cache folder on a crash |
| `VICTORY_CAPTURE=<dir>` | saves presented frames (powers of two, or `VICTORY_CAPTURE_FRAMES=<list>`) |
| `VICTORY_DUMP_TARGETS=1` | with the two above, saves every render target of those frames |
| `VICTORY_D3D12_DEBUG=1` | host debug layer |
| `VICTORY_INDIRECT=1` | runs the title's indirect draws and dispatches (off: the world's first frame stalls with it) |
| `VICTORY_D3D12_GBV=1` | with the above, GPU-based validation (bad descriptor use reported, not faulted; slow) |
| `VICTORY_D3D12_DRED=1` | on a GPU failure, the allocation hit and the commands in flight |
| `VICTORY_D3D12_WARP=1` | host device on the software adapter (slow; cannot reset the driver) |
| `VICTORY_AUDIO_CAPTURE=<file.wav>` | records what the title plays (our process only) |
| `VICTORY_XMA_DUMP=<dir>` | saves the first 32 XMA voices' context and input |

Every build appends crashes and host GPU failures to `crashes.log` in the
title's data folder.
