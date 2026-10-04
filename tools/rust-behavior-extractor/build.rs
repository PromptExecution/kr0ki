use std::{env, process::Command};

fn main() {
    let rustc = env::var("RUSTC").expect("Cargo provides rustc");
    let output = Command::new(rustc)
        .args(["--print", "sysroot"])
        .output()
        .expect("rustc sysroot");
    assert!(output.status.success(), "rustc sysroot failed");
    let sysroot = String::from_utf8(output.stdout).expect("UTF-8 sysroot");
    let lib = format!("{}/lib", sysroot.trim());
    println!("cargo:rustc-link-search=native={lib}");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{lib}");
}
