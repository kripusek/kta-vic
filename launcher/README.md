# GTA6 Compatibility Project (`kripusek/kta-vic`)

Projekt integrujący warstwę kompatybilności **VIctory** wraz z najnowszymi, aktywnymi forkami społecznościowymi pod NVIDIA PC (RTX 3060 Ti 8 GB).

## Profile w launcherze (`kripus_launcher.ps1`)

Launcher zawiera wyłącznie profile z **działającą grafiką 3D** (usunięto przestarzałe forki oraz profile bez oświetlonego świata):

| Profil | Źródło / Fork | Stan i uwagi |
|--------|---------------|--------------|
| **🌴 ScarlettPort v0.3.1 (ZALECANY)** | `ZigZagBoi1/ScarlettPort` (`925f90a`) | Najlepsze pokrycie shaderów postaci (skóra, włosy, ubrania), linkowanie 71 przerośniętych mesh shaderów, `VICTORY_PRECOMPILED_GRAPHICS=1`, KBM + AutoInput |
| **🚀 ScarlettPort — FastBoot + ReBAR** | `ZigZagBoi1/ScarlettPort` (`925f90a`) | Pomija logo/menu (`-FastBoot`), `VICTORY_REBAR=1` |
| **💎 NVIDIA 8GB (RTX 8GB)** | `soldsawn/victory-nvidia-8gb` | Optymalizacja pod karty 8 GB VRAM (`VICTORY_TEXTURE_DROP_MIPS=2`, `1920x1080`, Wave32, nakładka `F10`), **8–10 FPS** |
| **⚡ fxceless/victory-fork** | `fxceless/victory-fork` (`988aa41`) | Najnowszy build z nocnymi optymalizacjami slotów i `VICTORY_INDIRECT_GATHER=1`, **8–10 FPS** w prologu |

## Sprawdzone źródła (2026-11-26)
- `ZigZagBoi1/ScarlettPort` (`v0.3.1` @ `925f90a`)
- `soldsawn/victory-nvidia-8gb` (Wave32 + 8 GB VRAM)
- `fxceless/victory-fork` (`988aa41`)
- `Belobug/VIctory-Belobug-AMD` (`000ffdd` — wyłącznie AMD Radeon)
- `Marvisak/VIctory` (`upstream/main` @ `13f91cae`)

Szczegółowa dokumentacja znajduje się w [victory/README.md](victory/README.md).
