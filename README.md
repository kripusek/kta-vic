# VI Causeway

VI Causeway is an experimental Windows runtime for the Xbox Series X|S build of Grand Theft Auto VI. The game's x86-64 code runs on your CPU. Causeway implements the Xbox services it calls and translates its graphics commands and shaders to Direct3D 12. You'll need your own game files. Marvisak and the VIctory contributors built the original runtime. [Upstream VIctory](https://github.com/Marvisak/VIctory)

## What works

**The opening movie and first-time setup work on our RTX 3060 Ti. Playable story gameplay is still the goal.** We tested Xbox build `1.0.2.1` on Windows 10 22H2 with an i5-11600K. From the Player screen, the game plays the Rockstar animation with sound and saves first-time settings to a local profile. A returning profile goes straight from the movie to the illustrated story loading screens.

On a fresh profile, press **Menu** on the controller or **Esc** on the keyboard at **Finish Setup?** to choose **Settings**. We opened Control Scheme, Controller, Gameplay, and Display in the seven-tab menu and returned to setup with Back.

On Series X, the game showed **VICE CITY, TWO YEARS AGO** and **ROCKSTAR GAMES PRESENTS**. The scene between and after the cards stayed dark. In a 60-second run with a cold pipeline cache, ten world samples ranged from 0.6 to 1.4 FPS, with a median of 0.95 FPS. Earlier runs reset the display driver. We've also verified the offline Player picker at 1080p on Series S. We haven't verified visible world geometry, player control, or the in-story pause menu on this PC. [Story test and driver history](https://github.com/ifBars/VICauseway/pull/2) · [Series S compositor](https://github.com/Marvisak/VIctory/pull/211)

<details>
<summary>Earlier performance measurements</summary>

The opening movie ran at its original 30 FPS in a release build. Reusing texture row spans brought texture preparation down from 14.8–15.7 to 3.1–3.4 ms per movie frame. A process-only audio capture matched the soundtrack through its fade with no measurable accumulated delay.

In a CPU benchmark, checking a clean 6 MiB region inside a 174 MiB mapping fell from 327 to 4.3 microseconds. With one changed page, it fell from 19.7 to 9.2 microseconds. A later game build with additional renderer changes averaged 6.5% of the 12-thread CPU during the movie and 67.7% during the sampled world sequence. [Benchmark and upstream change](https://github.com/Marvisak/VIctory/pull/119)

On Windows 11 with a Radeon RX 6700 XT, upstream also reached the first mission card. The next scene stayed dark at 0.5–1.8 seconds per frame. [Upstream test](https://github.com/Marvisak/VIctory/blob/35b054ec038c11bc8a0de66b1eee764b1ad4f101/README.md#status-2026-11-23) · [Earlier NVIDIA device loss](https://github.com/Marvisak/VIctory/issues/71#issuecomment-7118495778) · [36-second NVIDIA test](https://github.com/Marvisak/VIctory/pull/135#issuecomment-7118921431)

</details>

## Build and run

Use Windows 10 or 11 x64, an AVX2 CPU, and a Direct3D 12 GPU with Shader Model 6.6. World rendering also uses mesh shaders. Install the Visual Studio C++ Build Tools, a Windows SDK, and the x64 MSVC Rust toolchain specified in [rust-toolchain.toml](rust-toolchain.toml).

1. From the repository root, install the asset-policy hook, copy the example configuration, and build the release executable.

    ```powershell
    cargo xtask install-hooks
    Copy-Item victory.example.toml victory.toml
    cargo build --locked --release -p victory --features agility
    ```

2. Open Microsoft's Agility SDK package as a ZIP. Copy `build/native/bin/x64/D3D12Core.dll` to `target\release\D3D12\D3D12Core.dll`. A diagnostic build uses the same directory layout beside its executable. For the Direct3D debug layer, add the matching `d3d12SDKLayers.dll` there too. [Microsoft.Direct3D.D3D12 1.620.2](https://www.nuget.org/packages/Microsoft.Direct3D.D3D12/1.620.2)

3. Edit `victory.toml` and set `[paths].game_dir` to your dump. Keep the game, cache, saves, and decoder table outside the checkout. For BCPack textures, set `bcpack_tables` to your own complete 77,982-byte table, or set `VICTORY_BCPACK_TABLES`. The build and synthetic tests run without it. [Example configuration](victory.example.toml) · [Asset policy](docs/asset-policy.md)

4. Check the paths, then launch the executable from your dump. This example selects Series S and caps the Windows output at 1080p. The executable and configuration keep their upstream names.

    ```powershell
    .\target\release\victory.exe config
    .\target\release\victory.exe run "D:\games\my-dump\game_scarlett_master.exe" --console series-s --output-height 1080 --execute
    ```

The window title shows the frame rate. By default, saves, settings, and `crashes.log` go to `%LOCALAPPDATA%\victory\data`. Set `data_dir` and `cache_dir` in the configuration to keep them on another drive.

### Launch options

| Option | When to use it |
| --- | --- |
| `--console series-x` or `--console series-s` | Select the console profile exposed to the game. |
| `--output-height 720` or `--output-height 1080` | Cap the Windows backbuffer height. The game still chooses its internal render targets. The flag takes precedence over `VICTORY_OUTPUT_HEIGHT`. |
| `--fps 30` or `--fps 60` | Override the title's 30/60 Hz presentation timing for this launch. Omit it for native timing. The setting does not change saves. |
| `--max-pipeline-workers 2` | Leave more CPU time for other programs during shader pipeline compilation. The i5-11600K normally uses six workers. A lower limit takes longer to warm the cache. |

At 60 Hz, the Player screen and opening movie submitted up to 60 frames a second. The movie still had 30 FPS source frames and normal-speed audio. The illustrated loading screen presented at 30 FPS.

The output cap preserves aspect ratio and uses the game's plane crops, placement, and alpha blending. WARP readback tests cover 1080p and 4K planes. If the compositor can't sample the primary plane, Causeway logs the fallback and copies it at native size when the resources match.

### Controls

Use an XInput controller, or use the keyboard while the game window has focus and no controller is connected.

| Keys | Controller input |
| --- | --- |
| W A S D / I J K L | Left / right stick |
| Arrow keys | D-pad |
| Enter, Backspace, X, Y | A, B, X, Y |
| Q / E, Z / C | Shoulders, triggers |
| F / H | Stick clicks |
| Esc, Tab | Menu, View |

## Commands

```
victory inspect <exe|dll> [--imports] [--exports]
victory census [paths...] [--missing] [--strict]
victory lib-exports <import.lib> [--list]
victory shader-info [paths...] [--verbose] [--disasm]
victory translate [paths...] [--validate] [--pipelines] [--list] [--jobs N]
victory run <exe> [--execute] [--unimplemented trap|zero] [--strict]
            [--console series-x|series-s] [--fps 30|60] [--output-height HEIGHT] [--max-pipeline-workers N]
            [-- <title arguments>]
victory config
```

Set `VICTORY_LOG=victory_loader=trace,info` to select log targets. Release builds include info, warning, and error messages.

On CPUs that emulate CLZERO, `VICTORY_BATCH_CLZERO=1` batches recognized 32 to 64 KiB clears in release and diagnostic builds. We haven't measured its effect on story frame time.

## Debugging

Build the diagnostic executable to capture frames, inspect failed draws, or run the GPU code on WARP. Run the workspace tests with synthetic data from the repository root.

```powershell
cargo build --locked --profile diag -p victory --features diagnostics,agility
cargo test --locked --workspace
```

| Variable | Effect |
|----------|--------|
| `VICTORY_LOG=<target>=debug,...` | debug and trace output |
| `VICTORY_STACKS=20,60` | every thread's stack at those seconds |
| `VICTORY_WATCH=<rva>,...` | logs a title u32 when it changes |
| `VICTORY_KEYS=30:Enter,35:X` | presses keys at those seconds, focus or not |
| `VICTORY_KEY_FILE=<path>` | reads one key per request, such as `Esc` or `Esc 2000` for a two-second hold. The default hold is 250 ms. |
| `VICTORY_CRASH_SAVE=<rva>:<len>,...` | saves title memory to the cache folder on a crash |
| `VICTORY_CAPTURE=<dir>` | saves frames numbered with powers of two and any frames listed in `VICTORY_CAPTURE_FRAMES` |
| `VICTORY_CAPTURE_ON_DEMAND=1` | saves listed frames plus one presented frame each time you create an empty `capture-next` file in the capture folder |
| `VICTORY_CAPTURE_FAST=1` | skips the per-frame black-transition probe and keeps requested PNG captures |
| `VICTORY_DUMP_TARGETS=1` | saves the render targets of explicitly listed capture frames. Use `1280x720-f28` (width, height, DXGI format) instead of `1` to save only matching targets alongside the unchanged presentation capture |
| `VICTORY_CAPTURE_DRAW_DROPS=1` | logs up to 4,096 operations recorded for the frame armed by `capture-target-next`. Each row gives the operation, pipeline, color and depth resource IDs (as in the PNG names), and the outcome or drop reason. The target selector controls the PNGs, while the trace also includes other draws and compute dispatches from that frame |
| `VICTORY_CAPTURE_INDIRECT_MESH=1` | samples up to 32 indirect mesh calls on the frame armed by `capture-target-next`. Each row identifies the recorded pipeline and reports up to 16 GPU-written group triples after clamping, bounded by the effective count. If the count lives in a separate buffer with unknown state, the row reports the sample as unavailable |
| `capture-target-next` | with a filtered target and `VICTORY_CAPTURE_ON_DEMAND=1`, create this empty file in the capture folder. It arms both PNG captures for two frame numbers after the present that consumes it, so matching targets can be retained before that frame is presented |
| `VICTORY_D3D12_DEBUG=1` | host debug layer |
| `VICTORY_D3D12_GBV=1` | enables GPU-based validation with the debug layer. Expect slower execution. |
| `VICTORY_D3D12_DRED=1` | on a GPU failure, the allocation hit and the commands in flight |
| `VICTORY_D3D12_WARP=1` | uses Microsoft's WARP software adapter for renderer checks |
| `VICTORY_GPU_TIMING=1` | times GPU command lists individually by submitting them serially |
| `VICTORY_AUDIO_CAPTURE=<file.wav>` | records what the title plays (our process only) |
| `VICTORY_XMA_DUMP=<dir>` | saves the first 32 XMA voices' context and input |

Every build appends crashes and host GPU failures to `crashes.log` in the
title's data folder.

## Credits and license

VI Causeway started on November 23, 2026 as an independent derivative of Marvisak's VIctory at `1dd7afd`. The imported history preserves author and co-author credits. Filtering out the upstream BCPack table changed the affected commit IDs. The compatibility layer uses GPL-3.0-or-later. [Original revision](https://github.com/Marvisak/VIctory/tree/1dd7afd4e68300e5dea41cd33a6e0b297798bb2f) · [License](LICENSE) · [How we bring in upstream changes](docs/upstream.md)

Two upstream fixes helped startup on NVIDIA. Nostyxx remapped descriptor tables that exceeded the host heap limit. Ryx (Ryx69) kept shader resource views stable until command-list reset. [Nostyxx's heap fix](https://github.com/Marvisak/VIctory/pull/21) · [Ryx69's descriptor fix](https://github.com/Marvisak/VIctory/pull/20)

## Dodane źródła, forki i integracje (`clean-base-v2`)

### Zewnętrzne repozytoria i forki
- **ZigZagBoi1/ScarlettPort**: `https://github.com/ZigZagBoi1/ScarlettPort` (remote `scarlettport`, lokalnie `e:\gta6\ScarlettPort`) — stos `runtime/victory` + D3D12 Agility SDK 1.614.1, tryb `VICTORY_FALLBACK_LIGHT=1` (oświetlenie zastępcze otwartego świata na RTX 3060), `-DebugView` oraz `-AutoInput`.
- **fxceless/victory-fork**: `https://github.com/fxceless/victory-fork` (remote `victoryfork`) — m.in. poprawka `e9765a7` wyłączająca domyślnie `VICTORY_WAVE_WIDE` (eliminacja device loss na `ptfx_CS_lightAtlas`).
- **igvcc/luacias**: `https://github.com/igvcc/luacias` (lokalnie `e:\gta6\luacias`) — baza `v0.2.0-preview`, dokumentacja i optymalizacje VRAM/LDS.
- **Corgansm/Corgi-victoryGTA6**: `https://github.com/Corgansm/Corgi-victoryGTA6` (remote `corgi`) — repozytorium śledzące.
- **Corgansm/victory-corgi**: `https://github.com/Corgansm/victory-corgi` (remote `victorycorgi`, lokalnie `e:\gta6\victory-corgi`) — fork oparty na `fxceless/victory-fork` z poprawką `e9765a7` (brak device loss na `ptfx_CS_lightAtlas`), translacją shadow/ray-tracing, prewarmem potoków oraz integracją `device.rs` / `renderdoc.rs`. Podpięty w launcherze jako **🐶 Victory Corgi**.
- **Marvisak/VIctory**: `https://github.com/Marvisak/VIctory` (remote `upstream`) — zintegrowane PR-y na `clean-base-v2`: `#93` (BCPack lookup), `#432` (uninitialized SGPRs), `#448` (min/max signed zeros), `#520` (`XAsync` payload), `#565` (conservative depth/position), `#637` (packed inline constants), `#654` (`uav-describe`), `#681` (`vertex-buffer-null-unbind`), `#706` (`cmp_lg_ordered`).
- **ifBars/VICauseway**: `https://github.com/ifBars/VICauseway` (remote `causeway`) — baza poranna `606a81b` + poprawki `8a05317`, `180ebb0`, `f681ba3` oraz PR-y `#360`, `#367`, `#368`, `#369`, `#1070`.

### Launcher i narzędzia lokalne (`e:\gta6\`)
- `e:\gta6\kripus_launcher.ps1` — główny launcher GUI (profile VIctory/Causeway, Tryb Diagnostyczny, ScarlettPort `FALLBACK_LIGHT=1`, ScarlettPort `DebugView`, integracja ze Steam).
- `e:\gta6\ODPAL_SCARLETTPORT.bat` — szybkie uruchamianie ScarlettPort.
- `e:\gta6\ODPAL_GTA6.bat` — szybkie uruchamianie VIctory/Causeway + Steam.
- `e:\gta6\dodaj_do_steama.py` / `DODAJ_DO_STEAMA.bat` — dodawanie skrótu GTA VI z okładkami do Steam.

## Poradnik uruchomienia krok po kroku (Tutorial)

### 1. Uruchomienie przez Launcher GUI (zalecane)
1. Otwórz launcher `e:\gta6\kripus_launcher.ps1` (kliknij prawym -> *Uruchom za pomocą programu PowerShell* lub skrótem).
2. Wybierz jeden z trybów na liście:
   - **🌴 ScarlettPort (RTX 3060) + Światło Zastępcze (`NOWE`)** — **zalecany na start**. Automatycznie przechodzi przez menu (`-AutoInput`) i włącza zastępcze oświetlenie otwartego świata (`VICTORY_FALLBACK_LIGHT=1`: niebo, słońce, mgła, budynki).
   - **🎨 ScarlettPort — Debug Color View (`-DebugView`)** — wyświetla surowe bufory geometrii 3D w kolorach debugowych.
   - **🎮 Xbox Series S @ 1080p 60 FPS** — klasyczny runtime `VIctory/Causeway` (`clean-base-v2`).
   - **🛠️ Tryb Diagnostyczny** — uruchamia `diag\victory.exe` z warstwą debugowania D3D12 i pełnym logowaniem.
3. Kliknij **URUCHOM GRĘ**.
4. **Cierpliwości przy pierwszym ładowaniu:** kompilacja shaderów po menu może zająć kilka minut, a w otwartym świecie FPS na RTX 3060/3060 Ti spada obecnie do ok. **~1 FPS**. Dopóki w konsoli pojawiają się logi, gra pracuje i nie należy jej zamykać.

### 2. Szybkie uruchomienie bez GUI (`.bat`)
- **ScarlettPort:** uruchom `e:\gta6\ODPAL_SCARLETTPORT.bat` i wciśnij `Enter` (opcja `[1] AutoInput + Światło zastępcze`).
- **VIctory / Causeway:** uruchom `e:\gta6\ODPAL_GTA6.bat` i wybierz opcję `[1]` (Series S 1080p), `[5]` (Tryb Diagnostyczny) lub `[6]` (Steam).

### 3. Integracja ze Steam (status *"W grze: Grand Theft Auto VI"*)
1. W launcherze kliknij przycisk **Dodaj / Odśwież w Steam** (lub uruchom `e:\gta6\DODAJ_DO_STEAMA.bat`).
2. Zrestartuj aplikację Steam (zamknij całkowicie z zasobnika systemowego i uruchom ponownie).
3. Gra pojawi się w bibliotece Steam z okładką, banerem i ikoną.

### 4. Sterowanie
- **Pad Xbox (XInput):** działa natywnie od razu po uruchomieniu.
- **Klawiatura:** `W A S D` (lewy analog), `I J K L` (prawy analog), `Enter` (`A`), `Backspace` (`B`), `X` / `Y`, `Q` / `E` (bumpery), `Z` / `C` (triggery), `Esc` (Menu).

### 5. Gdzie szukać logów i jak rozwiązywać problemy
- **Logi ScarlettPort:** `e:\gta6\ScarlettPort\logs\victory-*.log`
- **Logi crashy VIctory/Causeway:** `E:\gta6\victory-data\data\7952987A\crashes.log`
- Jeśli po aktualizacji pojawi się błąd GPU lub zawieszenie w menu, użyj przycisku **Wyczyść Cache Shaderów** w launcherze lub uruchom **Tryb Diagnostyczny** (`[5]`), który zapisze dokładny powód w `crashes.log`.
