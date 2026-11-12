use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    for (var, name) in [
        ("ZEROWOL_AGENT_X86_64", "agent-x86_64"),
        ("ZEROWOL_AGENT_AARCH64", "agent-aarch64"),
    ] {
        println!("cargo:rerun-if-env-changed={var}");
        let dest = out.join(name);
        match std::env::var_os(var).filter(|v| !v.is_empty()) {
            Some(path) => {
                println!("cargo:rerun-if-changed={}", PathBuf::from(&path).display());
                std::fs::copy(&path, &dest)
                    .unwrap_or_else(|e| panic!("cannot copy {var}={path:?}: {e}"));
            }
            None => std::fs::write(&dest, b"").expect("write empty agent placeholder"),
        }
    }
}
