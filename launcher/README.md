# GTA6 Compatibility Project (`kripusek/kta-vic`)

Projekt integrujący warstwę kompatybilności **VIctory** wraz z najnowszymi, aktywnymi forkami społecznościowymi pod Windows PC.

## Profile w launcherze (`kripus_launcher.ps1`)

Launcher zawiera wyłącznie profile z **działającą grafiką 3D** oraz wbudowany **📊 Analizator gry na żywo** (etap, FPS, RAM, shadery, błędy, podgląd logów):

| Profil | Źródło / Fork | Stan i uwagi |
|--------|---------------|--------------|
| **🦊 CGBJ / cool_fox (`pareto-residency`)** | `cputroll/cgbj-reupload` (`f4b48ad`) / `Pavl3000/cgbj-fork` | Najnowszy przełom: **13–22 FPS** (`VICTORY_THIN=1`, `JOIN_PARTS=8`, `CARD_STEPS=2`, `AMBIENT=0.8`, `Series S` + RTX residency, sesje do 41 min bez crasha) |
| **🚀 ScarlettPort — FastBoot + Experimental** | `ZigZagBoi1/ScarlettPort` (`925f90a`) | `-FastBoot -AutoInput` + `REBAR=1` + `INDIRECT_WINDOW/KEEP` |
| **🌴 ScarlettPort v0.3.1** | `ZigZagBoi1/ScarlettPort` (`925f90a`) | Pełna prekompilacja potoków (`25 200`), najlepsze pokrycie shaderów postaci |
| **💎 NVIDIA 8GB (Series S + Mip Drop=2)** | `soldsawn/victory-nvidia-8gb` | Optymalizacja pod karty 8 GB VRAM (`1920x1080`, Wave32, nakładka `F10`) |
| **⚡ fxceless/victory-fork** | `fxceless/victory-fork` (`988aa41`) | Build z `VICTORY_INDIRECT_GATHER=1` (wymuszony `series-s`) |

## Sprawdzone źródła (2026-11-26)
- `cputroll/cgbj-reupload` (`f4b48ad` — mirror `codec-d/cool_fox`)
- `Pavl3000/cgbj-fork` (`2eb9b22` — mirror z commitami z `avatarmanz/VIctory-7900XTX`)
- `ZigZagBoi1/ScarlettPort` (`v0.3.1` @ `925f90a`)
- `soldsawn/victory-nvidia-8gb`
- `fxceless/victory-fork` (`988aa41`)

Szczegółowa dokumentacja znajduje się w [victory/README.md](victory/README.md).
