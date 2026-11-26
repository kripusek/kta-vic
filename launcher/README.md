# GTA6 Compatibility Project (`kripusek/kta-vic`)

Projekt integrujący warstwę kompatybilności **VIctory** wraz z najnowszymi, aktywnymi forkami społecznościowymi pod Windows PC (NVIDIA RTX).

## Profile w launcherze (`kripus_launcher.ps1`)

Launcher zawiera wyłącznie profile z **działającą grafiką 3D** oraz wbudowany **📊 Analizator gry na żywo** (etap, FPS, RAM, shadery, błędy, podgląd logów):

| Profil | Źródło / Fork | Stan i uwagi |
|--------|---------------|--------------|
| **🚀 VIctory-NVIDIA (~20 FPS + pełne drawy)** *(domyślny)* | `trustno1-coder/VIctory-NVIDIA` | Naprawa limitu deskryptorów NVIDIA (`VICTORY_DYNAMIC_SLOTS=524288`), `Series S` 0.5x + `OWN_UPSCALER`, `NAN_SCRUB=1`, `BVH_MISS=1`, porównania pamięci na GPU (~20–22 FPS na RTX 5070) |
| **🎨 VIctory-NVIDIA Quality (~13 FPS)** | `trustno1-coder/VIctory-NVIDIA` | Tryb `-Quality`: mniej rozciągniętych ciał i tęczowej skóry |
| **🦊 DoubleGhosty/cool_fox** | `DoubleGhosty/cool_fox` | Kontynuacja CGBJ (`Pareto residency`, LOD fix, sync wideo NVIDIA) + `VICTORY_OWN_UPSCALER=1` |
| **🌴 ScarlettPort v0.4.5 (Look + FastBoot + ReBAR)** | `ZigZagBoi1/ScarlettPort` (`f300de1`) | Nowy silnik `engine/scarlett.exe`, preset `-Look`, `-FastBoot -AutoInput`, `REBAR=1` |
| **⚡ fxceless/victory-fork (`e4245f3`)** | `fxceless/victory-fork` (`e4245f3`) | Naprawa czarnych twarzy, ruszające się usta (PR #927), `-10 ms/klatkę`, wymuszony `series-s` |

Szczegółowa dokumentacja znajduje się w [victory/README.md](victory/README.md).
