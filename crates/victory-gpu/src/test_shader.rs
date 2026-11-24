use std::path::{Path, PathBuf};

/// The PC `dxc.exe` (`VICTORY_PC_DXC`, else the newest Windows SDK's).
pub(crate) fn dxc() -> Option<PathBuf> {
    let dxc = match std::env::var_os("VICTORY_PC_DXC") {
        Some(p) => PathBuf::from(p),
        None => {
            victory_shader::emit::validator::Validator::sdk_tool("dxc.exe")?
        }
    };
    dxc.exists().then_some(dxc)
}

pub(crate) fn compile(dxc: &Path, dir: &Path, src: &str, profile: &str) -> Vec<u8> {
    let (hlsl, out) = (
        dir.join(format!("{profile}.hlsl")),
        dir.join(format!("{profile}.dxil")),
    );
    std::fs::write(&hlsl, src).unwrap();
    let status = std::process::Command::new(dxc)
        .args(["-nologo", "-T", profile, "-E", "main", "-Fo"])
        .arg(&out)
        .arg(&hlsl)
        .status()
        .unwrap();
    assert!(status.success(), "dxc failed for {profile}");
    std::fs::read(out).unwrap()
}
