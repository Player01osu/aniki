use std::path::{Path, PathBuf};

mod parser;

#[derive(Debug, Eq, PartialEq)]
pub struct Config {
    thumbnail_path: PathBuf,
    database_path: PathBuf,
    video_paths: Vec<PathBuf>,
}
struct EnvVars {
    home: String,
    xdg_config_home: Option<String>,
    xdg_cache_home: Option<String>,
    aniki_config: Option<String>,
}

impl EnvVars {
    fn new() -> Self {
        Self {
            home: std::env::var("HOME").unwrap(),
            xdg_config_home: std::env::var("XDG_CONFIG_HOME").ok(),
            xdg_cache_home: std::env::var("XDG_CACHE_HOME").ok(),
            aniki_config: std::env::var("ANIKI_CONFIG").ok(),
        }
    }

    fn xdg_cache(&self) -> PathBuf {
        if let Some(path) = &self.xdg_cache_home {
            Path::new(path).join("aniki")
        } else {
            Path::new(&self.home).join("aniki")
        }
    }

    fn xdg_config(&self) -> PathBuf {
        if let Some(path) = &self.xdg_config_home {
            Path::new(path).join("aniki")
        } else {
            Path::new(&self.home).join("aniki")
        }
    }
}

fn create_if_not_exist(p: impl AsRef<Path>) {
    let p = p.as_ref();
    if !p.exists() {
        std::fs::create_dir_all(p).unwrap();
    }
}

impl Config {
    pub fn parse_cfg() -> Self {
        let raw = Self::parse_cfg_raw();
        create_if_not_exist(&raw.thumbnail_path);
        create_if_not_exist(raw.database_path.parent().unwrap());
        raw
    }

    fn parse_cfg_raw() -> Self {
        let env_vars = EnvVars::new();

        // TODO: Windows support
        let base_dir_path = env_vars.xdg_cache();
        let database_path = base_dir_path.join("aniki.db");
        let thumbnail_path = base_dir_path.join("thumbnails");
        let video_paths = vec![];
        let config_path = match (&env_vars.aniki_config, &env_vars.xdg_config_home) {
            (Some(ref path), _) if Path::new(path).exists() => PathBuf::from(path),
            (_, Some(ref xdg_config_home))
                if Path::new(xdg_config_home).join("aniki/aniki.conf").exists() =>
            {
                Path::new(xdg_config_home).join("aniki/aniki.conf")
            }
            _ if Path::new(&env_vars.home).join("aniki.conf").exists() => {
                Path::new(&env_vars.home).join("aniki.conf")
            }
            _ => return Self::default_config(&env_vars),
        };
        Self::parse(config_path, thumbnail_path, database_path, video_paths)
    }

    pub fn thumbnail_path(&self) -> &PathBuf {
        &self.thumbnail_path
    }

    pub fn database_path(&self) -> &PathBuf {
        &self.database_path
    }

    pub fn video_paths(&self) -> &[PathBuf] {
        &self.video_paths
    }

    fn default_config(env_vars: &EnvVars) -> Self {
        let base_dir_path = Path::new(&env_vars.home).join("aniki");
        let database_path = base_dir_path.join("aniki.db");
        let thumbnail_path = base_dir_path.join("thumbnails");
        let video_paths = vec![];
        Self {
            thumbnail_path,
            database_path,
            video_paths,
        }
    }
}
