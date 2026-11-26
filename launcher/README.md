# GTA6 Compatibility Project (`kripusek/kta-vic`)

Projekt integrujący warstwę kompatybilności **VIctory** wraz z aktywnymi forkami społecznościowymi pod Windows PC (RTX 3060 Ti 8 GB).

## Profile w launcherze (`kripus_launcher.ps1`)

Launcher zawiera wyłącznie działające profile oraz wbudowany **📊 Analizator gry na żywo** (etap, FPS, RAM, shadery, błędy, podgląd logów):

| Profil | Źródło / Fork | Stan i uwagi |
|--------|---------------|--------------|
| **🦊 CGBJ / cool_fox (`pareto-residency` + `OWN_UPSCALER=1`)** *(domyślny)* | `cputroll/cgbj-reupload` (`f4b48ad`) + patch `OWN_UPSCALER=1` | Sprawdzony na RTX 3060 Ti: najwyższy klatkaż (6–10 FPS), brak crasha FSR4 |
| **🦊 DoubleGhosty/cool_fox** | `DoubleGhosty/cool_fox` | Kontynuacja CGBJ (`Pareto residency`, LOD fix, sync wideo NVIDIA) + `VICTORY_OWN_UPSCALER=1` |
| **🌴 ScarlettPort v0.4.5 (Look + FastBoot + ReBAR)** | `ZigZagBoi1/ScarlettPort` (`f300de1`) | Nowy silnik `engine/scarlett.exe`, preset `-Look`, `-FastBoot -AutoInput`, `REBAR=1` |
| **⚡ fxceless/victory-fork (`e4245f3`)** | `fxceless/victory-fork` (`e4245f3`) | Naprawa czarnych twarzy, ruszające się usta (PR #927), `-10 ms/klatkę`, wymuszony `series-s` |

Szczegółowa dokumentacja znajduje się w [victory/README.md](victory/README.md).
