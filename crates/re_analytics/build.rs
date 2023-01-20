fn main() {
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let target = std::env::var("TARGET").unwrap();
    let dest_path = std::path::Path::new(&out_dir).join("target.rs");
    std::fs::write(&dest_path, format!("\"{target}\"")).unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}
