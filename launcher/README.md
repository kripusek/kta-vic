# GTA6 Compatibility Project (`kripusek/kta-vic`)

Projekt integrujący warstwę kompatybilności **VIctory** wraz z aktywnymi forkami społecznościowymi do uruchamiania GTA VI (Xbox Series X|S) na Windows PC.

## Podział w launcherze (`kripus_launcher.ps1`)

Launcher GUI dzieli profile na dwie wyraźne kategorie:

### ✅ Działające graficznie (oświetlony świat / render 3D)
| Profil | Źródło / Fork | Stan i uwagi |
|--------|---------------|--------------|
| **ScarlettPort v0.3.1** | `ZigZagBoi1/ScarlettPort` (`925f90a`) | Oświetlony świat (`VICTORY_OWN_UPSCALER=1`, `VICTORY_PRECOMPILED_GRAPHICS=1`), KBM + AutoInput |
| **ScarlettPort — FastBoot + ReBAR** | `ZigZagBoi1/ScarlettPort` (`925f90a`) | Pomija logo/menu (`-FastBoot`), `VICTORY_REBAR=1` |
| **fxceless/victory-fork** | `fxceless/victory-fork` (`085ee7c4`) | **8–10 FPS** w pierwszych pomieszczeniach prologu (`VICTORY_INDIRECT_GATHER=1`, pełny zestaw flag wydajnościowych) |
| **Belobug AMD Fork** | `Belobug/VIctory-Belobug-AMD` (`000ffdd4`) | Oświetlenie ambient (`VICTORY_AMBIENT=0.6`), pojazdy w ruchu, dalekie drzewa, nakładka FPS/frametime |
| **Victory Corgi** | `Corgansm/victory-corgi` | Series X (po starcie ekran może być chwilowo czarny; na `PLAYER` naciśnij Enter/A) |

### ⚠️ Nie działające graficznie (czarny świat po intro / diagnostyka)
| Profil | Źródło / Fork | Stan i uwagi |
|--------|---------------|--------------|
| **VI Causeway / kta-vic — Series S 1080p** | `kripusek/kta-vic` | Intro i menu działają, świat 3D po kartach ładowania pozostaje ciemny |
| **VI Causeway / kta-vic — Series S Unlocked** | `kripusek/kta-vic` | Brak limitu 60 FPS (świat 3D nadal nieoświetlony) |
| **VI Causeway / kta-vic — Series X 1080p** | `kripusek/kta-vic` | Profil Series X (świat 3D nieoświetlony) |
| **VI Causeway / kta-vic — Series X 4K** | `kripusek/kta-vic` | Natywne 4K (świat 3D nieoświetlony) |
| **Tryb Diagnostyczny** | `kripusek/kta-vic` (`\diag\victory.exe`) | D3D12 Debug + DRED + `victory_loader=trace` |

## Sprawdzone źródła (2026-11-25)
- `Marvisak/VIctory` (`upstream/main` @ `13f91cae`) — prolog osiągalny przy ~1 FPS
- `fxceless/victory-fork` (`085ee7c4`) — 8–10 FPS w prologu, `VICTORY_SHOT`, `VICTORY_INDIRECT_GATHER=1`
- `Belobug/VIctory-Belobug-AMD` (`000ffdd4`) — thin LTO, ambient + refleksje, nakładka FPS, optymalizacje deskryptorów
- `ZigZagBoi1/ScarlettPort` (`925f90a` / `v0.3.1`) — nowy launcher 0.3.1, prekompilowane potoki graficzne
- `igvcc/luacias` (`238d68f`) — `v0.2.0-preview`
- `Corgansm/victory-corgi` — Series X + `ptfx_CS_lightAtlas` fix

Szczegółowa dokumentacja znajduje się w [victory/README.md](victory/README.md).
