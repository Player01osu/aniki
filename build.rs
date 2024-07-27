use std::env;

const JSON_DATABASE_UPSTREAM: &str = "https://github.com/manami-project/anime-offline-database/raw/master/anime-offline-database-minified.json";
const JSON_DATABASE_PATH: &str = "./anime-offline-database.json";

fn dl_database(path: &std::path::Path) -> Result<(), String> {
    let data = reqwest::blocking::get(JSON_DATABASE_UPSTREAM)
        .map_err(|e| e.to_string())?
        .bytes()
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, data).unwrap();
    Ok(())
}

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let path = std::path::Path::new(&out_dir).join(JSON_DATABASE_PATH);
    if !path.exists() {
        if let Err(e) = dl_database(&path) {
            eprintln!("could not download database:{e}");
            std::process::exit(1);
        }
    }

    //#[cfg(target_os = "linux")]
    //println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");

    println!("cargo:rerun-if-changed={}", path.to_string_lossy());
}
