use std::env;

const JSON_DATABASE_UPSTREAM: &str = "https://github.com/manami-project/anime-offline-database/raw/master/anime-offline-database-minified.json";
const JSON_DATABASE_PATH: &str = "./anime-offline-database.json";

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let path = std::path::Path::new(&out_dir).join(JSON_DATABASE_PATH);
    if !path.exists() {
        let data = reqwest::blocking::get(JSON_DATABASE_UPSTREAM)
            .unwrap()
            .bytes()
            .unwrap();
        std::fs::write(&path, data).unwrap();
    }

    println!("cargo:rerun-if-changed={}", path.to_string_lossy());
}
