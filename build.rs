use std::{env, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/app.ico");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let resource = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("app.res");
    let result = Command::new("rc.exe")
        .arg("/nologo")
        .arg("/fo")
        .arg(&resource)
        .arg("assets/app.rc")
        .status()
        .expect("No se encontro rc.exe. Compila desde x64 Native Tools Command Prompt.");
    assert!(result.success(), "No se pudo compilar el recurso del icono");
    println!("cargo:rustc-link-arg={}", resource.display());
}
