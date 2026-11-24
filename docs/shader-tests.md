# Testing shaders on Windows

`cargo test -p victory-shader` runs the shader translation checks. The wave tests also dispatch compute shaders and draw a pixel on each available Shader Model 6.6 hardware or WARP device. When neither device supports a test, the default suite skips its GPU assertions. Pass `-- --nocapture` to see these messages.

For a run that requires GPU coverage, use the x64 MSVC toolchain and the Microsoft D3D12 Agility SDK 620. The x64 `D3D12Core.dll` from Microsoft's `Microsoft.Direct3D.D3D12` package version `1.620.2` goes under `debug\deps\D3D12\` in your Cargo target directory. Cargo runs the shader tests from `debug\deps`.

```powershell
cargo test --locked -p victory-shader --features agility --lib emit::wave_tests:: -- --nocapture
```

`cargo test --workspace --features victory/agility` selects the same SDK for the shader test process. These commands fail a wave test if it cannot execute on any compatible device. Use `release\deps\D3D12\` under the target directory when running tests with `--release`.
