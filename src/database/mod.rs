pub mod episode;
pub mod json_database;
pub mod sanitize;

use anyhow::Context;
use episode::Episode;
use flexbuffers::{DeserializationError, SerializationError};
use std::fs::{metadata, read_dir, DirEntry, File};
use std::io::Write;
use std::path::PathBuf;
use std::time::SystemTimeError;
use std::{path::Path, time::SystemTime};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use walkdir::WalkDir;

use crate::anilist_serde::{Collection, Media, MediaEntry};

use self::json_database::{AnimeDatabaseData, JsonIndexed};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anime {
    filename: String,
    paths: Box<[String]>,
    last_watched: u64,
    last_updated: u64,
    current_episode: Episode,
    episodes: EpisodeMap,

    thumbnail: Option<String>,
    alias: Option<String>,

    // From JSON Database
    metadata: Option<AnimeDatabaseData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AniListCred {
    user_id: u64,
    access_token: Box<str>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Database<'a> {
    anime_map: Vec<Anime>,
    previous_update: Vec<(Box<str>, u64)>,
    skip_login: bool,
    anilist_cred: Option<AniListCred>,
    #[serde(skip)]
    indexed_db: Option<JsonIndexed<'a>>,
    #[serde(skip)]
    cached_view: CachedView<'a>,
    #[serde(skip)]
    anilist_collections: Option<Box<[Media]>>,
}

#[derive(Debug, Default)]
struct CachedView<'a> {
    last_updated: u64,
    animes: Vec<&'a mut Anime>,
}

pub type EpisodeMap = Vec<(Episode, Vec<String>)>;

#[derive(Debug, Error)]
pub enum InvalidEpisodeError {
    #[error("{episode} Does not exist in \"{anime}\"")]
    NotExist { anime: String, episode: Episode },
}

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("{0}")]
    IO(std::io::Error),
    #[error("{0}")]
    Deserialization(DeserializationError),
    #[error("{0}")]
    Serialization(SerializationError),
    #[error("{0}")]
    SystemTime(SystemTimeError),
    #[error("Invalid path to episode")]
    InvalidFile,
    #[error("Unable to convert file to UTF-8 string")]
    UTF8,
    #[error("{0}")]
    InvalidEpisode(InvalidEpisodeError),
}

type Err = DatabaseError;

impl From<std::io::Error> for Err {
    fn from(v: std::io::Error) -> Self {
        Self::IO(v)
    }
}

impl From<SystemTimeError> for Err {
    fn from(v: SystemTimeError) -> Self {
        Self::SystemTime(v)
    }
}

impl From<DeserializationError> for Err {
    fn from(v: DeserializationError) -> Self {
        Self::Deserialization(v)
    }
}

impl From<SerializationError> for Err {
    fn from(v: SerializationError) -> Self {
        Self::Serialization(v)
    }
}

type Result<T> = std::result::Result<T, Err>;

macro_rules! o_to_str {
    ($x: expr) => {
        $x.to_str().unwrap().to_string()
    };
}

fn get_time() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn is_empty_dir_entry(dir_entry: &DirEntry) -> bool {
    is_empty_dir(dir_entry.path())
}

fn is_empty_dir(path: PathBuf) -> bool {
    path.read_dir()
        .map(|mut v| v.next().is_none())
        .unwrap_or(false)
}

impl Anime {
    pub fn from_path(
        path: impl AsRef<Path>,
        file_name: String,
        metadata: Option<AnimeDatabaseData>,
        time: u64,
    ) -> Self {
        let path = path.as_ref();
        let mut anime = Anime {
            filename: file_name,
            paths: vec![o_to_str!(path).into()].into(),
            last_watched: 0,
            last_updated: time,
            current_episode: Episode::from((1, 1)),
            episodes: Vec::new(),
            thumbnail: None,
            alias: None,
            metadata,
        };
        anime.update_episodes();
        anime
    }

    pub fn paths(&self) -> &Box<[String]> {
        &self.paths
    }

    pub fn metadata(&self) -> &Option<AnimeDatabaseData> {
        &self.metadata
    }

    pub fn thumbnail(&self) -> &Option<String> {
        &self.thumbnail
    }

    pub fn set_thumbnail(&mut self, path: Option<String>) {
        self.thumbnail = path;
    }

    pub fn title(&self) -> &str {
        self.metadata
            .as_ref()
            .map(|m| m.title())
            .unwrap_or(&self.filename)
    }

    pub fn display_title(&self) -> &str {
        self.alias
            .as_ref()
            .map(String::as_str)
            .unwrap_or_else(|| self.title())
    }

    pub fn set_alias(&mut self, s: String) {
        self.alias = Some(s);
    }

    pub fn set_metadata(&mut self, metadata: Option<AnimeDatabaseData>) {
        self.metadata = metadata;
    }

    pub fn set_last_watched(&mut self, time: u64) {
        self.last_watched = time;
    }

    pub fn last_watched(&self) -> u64 {
        self.last_watched
    }

    pub fn as_ptr_id(&self) -> u64 {
        (self as *const Anime) as u64
    }

    fn set_progress(&mut self, progress: u32) {
        let mut guess = None;
        let weigh_guess = |enumeration: u32, episode: Option<u32>| {
            let enum_diff = enumeration.abs_diff(progress);
            episode
                .map(|n| enum_diff.abs_diff(n.abs_diff(progress)))
                .unwrap_or(enum_diff)
        };

        let mut replace_weight = |episode_struct: &Episode, enumeration, episode| {
            let weight = weigh_guess(enumeration, episode);
            if let Some((prev_weight, _)) = guess {
                if prev_weight > weight {
                    guess = Some((weight, episode_struct.clone()));
                }
            } else {
                guess = Some((weight, episode_struct.clone()));
            }
        };

        for (n, (episode_struct, _)) in self.episodes.iter().enumerate() {
            let n = n as u32 + 1; // Enumerate from 1

            match episode_struct {
                Episode::Numbered { episode, .. } if n == progress && *episode == progress => {
                    self.current_episode = episode_struct.clone();
                    return;
                }
                Episode::Numbered { episode, .. } if n == progress => {
                    replace_weight(&episode_struct, n, Some(*episode));
                }
                _ if n == progress => {
                    replace_weight(&episode_struct, n, None);
                }
                _ => (),
            }
        }

        if let Some((_, guess)) = guess {
            self.current_episode = guess;
        }
    }

    pub fn len(&self) -> usize {
        self.episodes.len()
    }

    pub fn anilist_id(&self) -> Option<u32> {
        let metadata = match self.metadata() {
            Some(v) => v,
            None => return None,
        };

        for source in metadata.sources() {
            let url = reqwest::Url::parse(source.as_str()).expect("Valid url");
            if let Some("anilist.co") = url.domain() {
                return Some(
                    url.path()
                        .chars()
                        .filter(char::is_ascii_digit)
                        .collect::<String>()
                        .parse::<u32>()
                        .unwrap(),
                );
            }
        }
        None
    }

    pub fn update_episodes(&mut self) {
        for path in self.paths.iter() {
            WalkDir::new(path)
                .max_depth(5)
                .min_depth(1)
                .into_iter()
                .filter_map(|d| d.ok()) // Report directory not found
                .filter(|d| {
                    d.file_type().is_file()
                        && d.path()
                            .extension()
                            .map(|e| matches!(e.to_str(), Some("mkv") | Some("mp4") | Some("ts")))
                            .unwrap_or(false)
                })
                .filter_map(|dir_entry| {
                    let episode = Episode::try_from(dir_entry.path()).ok()?;
                    let path = dir_entry.path().to_str()?.to_owned();

                    Some((episode, path))
                })
                .for_each(
                    |(ep, path)| match self.episodes.iter_mut().find(|(v, _)| ep.eq(v)) {
                        Some((_, paths)) => paths.push(path.clone()),
                        None => self.episodes.push((ep, vec![path])),
                    },
                );
            self.episodes.sort_by(|(a, _), (b, _)| a.cmp(b));
            if !self.has_episode(&self.current_episode) {
                if let Some((ref episode, _)) = self.episodes.get(0) {
                    self.current_episode = episode.clone();
                }
            }
        }
    }

    pub fn has_episode(&self, episode: &Episode) -> bool {
        self.episodes.iter().any(|(v, _)| episode.eq(v))
    }

    pub fn find_episode_path(&self, episode: &Episode) -> &[String] {
        self.episodes
            .iter()
            .find(|(v, _)| episode.eq(v))
            .map(|(_, v)| v.as_slice())
            .unwrap_or_else(|| &[])
    }

    pub fn has_next_episode(&self) -> bool {
        self.next_episode().is_some()
    }

    pub fn current_episode(&self) -> Episode {
        self.current_episode.clone()
    }

    pub fn next_episode(&self) -> Option<Episode> {
        match self.current_episode {
            Episode::Numbered { season, episode } => self.next_episode_raw((season, episode)),
            Episode::Special { .. } => None,
        }
    }

    pub fn next_episode_raw(
        &self,
        _current_episode @ (season, episode): (u32, u32),
    ) -> Option<Episode> {
        let get_episode = |season, episode| {
            self.episodes
                .iter()
                .find(|(ep, _)| ep.eq(&Episode::Numbered { season, episode }))
                .map(|v| v.0.clone())
        };

        if let Some(episode) = get_episode(season, episode + 1) {
            Some(episode)
        } else if let Some(episode) = get_episode(season + 1, 0) {
            Some(episode)
        } else {
            get_episode(season + 1, 1)
        }
    }

    pub fn filename(&self) -> &str {
        &self.filename
    }

    /// Gets current episode of directory in (season, episode) form.
    pub fn current_episode_path(&self) -> (Episode, &[String]) {
        (
            self.current_episode.clone(),
            self.find_episode_path(&self.current_episode),
        )
    }

    pub fn next_episode_path(&self) -> Result<Option<(Episode, &[String])>> {
        match self.current_episode {
            Episode::Numbered { season, episode } => {
                Ok(self.next_episode_path_raw((season, episode)))
            }
            Episode::Special { .. } => Ok(None),
        }
    }

    pub fn next_episode_path_raw(
        &self,
        _current_episode @ (season, episode): (u32, u32),
    ) -> Option<(Episode, &[String])> {
        let get_episode = |season, episode| {
            self.episodes
                .iter()
                .find(|(ep, _)| ep.eq(&Episode::Numbered { season, episode }))
                .map(|v| v.0.clone())
        };

        if let Some(episode) = get_episode(season, episode + 1) {
            let paths = self.find_episode_path(&episode);
            Some((episode, paths))
        } else if let Some(episode) = get_episode(season + 1, 0) {
            let paths = self.find_episode_path(&episode);
            Some((episode, paths))
        } else if let Some(episode) = get_episode(season + 1, 1) {
            let paths = self.find_episode_path(&episode);
            Some((episode, paths))
        } else {
            None
        }
    }

    pub fn episodes<'a>(&self) -> &'a EpisodeMap {
        unsafe { &*(&self.episodes as *const _) }
    }

    /// Prefer `.update_watched` because it checks if episode exists in episode_map.
    pub unsafe fn update_watched_unchecked(&mut self, watched: Episode) {
        let timestamp = get_time();
        self.last_watched = timestamp;
        self.current_episode = watched;
    }

    pub fn update_watched(&mut self, watched: Episode) -> Result<()> {
        match self.episodes.iter().find(|(ep, _)| watched.eq(ep)) {
            Some(_) => {
                unsafe { self.update_watched_unchecked(watched) };
                Ok(())
            }
            None => Err(Err::InvalidEpisode(InvalidEpisodeError::NotExist {
                anime: format!("{:?}", self.paths),
                episode: watched,
            })),
        }
    }
}

fn dir_modified_time(path: impl AsRef<Path>) -> Result<u64> {
    match metadata(path) {
        Ok(v) => Ok(v
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs()),
        Err(_) => Ok(0),
    }
}

fn download_image(url: &str, path: &str) -> anyhow::Result<()> {
    eprintln!("Retrieving images...");
    let url = url.to_owned();
    let path = path.to_owned();
    tokio::task::spawn(async {
        let data = reqwest::get(url)
            .await
            .context("Failed to connect to url")?
            .bytes()
            .await?;
        tokio::fs::write(path, data)
            .await
            .context("Failed to write to file")?;
        Ok::<(), anyhow::Error>(())
    });
    Ok(())
}

impl AniListCred {
    pub fn new(user_id: u64, access_token: String) -> Self {
        Self {
            user_id,
            access_token: access_token.into(),
        }
    }

    pub fn user_id(&self) -> u64 {
        self.user_id
    }

    pub fn access_token(&self) -> &str {
        &self.access_token
    }
}

impl<'a> Database<'a> {
    pub fn new(path: impl AsRef<str>, anime_directories: Vec<impl AsRef<str>>) -> Result<Self> {
        let path = path.as_ref();
        let mut db = match std::fs::read(path) {
            Ok(v) => {
                let mut db = flexbuffers::from_slice::<Self>(&v)?;

                // Check if directory has been updated
                for directory in anime_directories.iter() {
                    let directory = directory.as_ref();
                    let last_modified = dir_modified_time(directory)?;
                    let mut buf = String::new();

                    if !Path::new(directory).exists() {
                        continue;
                    }

                    match db
                        .previous_update
                        .iter()
                        .find(|(s, _)| directory.eq(s.as_ref()))
                    {
                        Some((_, last_updated)) => {
                            // Updated directory
                            if *last_updated < last_modified {
                                db.update_directory(directory, get_time(), &mut buf);
                            }
                        }
                        // Added new directory
                        None => {
                            db.update_directory(directory, get_time(), &mut buf);
                        }
                    }
                }
                db
            }
            Err(_) => {
                let mut db = Self {
                    anime_map: vec![],
                    previous_update: vec![],
                    skip_login: false,
                    anilist_cred: None,
                    indexed_db: None,
                    cached_view: CachedView::default(),
                    anilist_collections: None,
                };
                db.update(anime_directories);
                db
            }
        };
        db.update_cached();
        db.anime_map.sort_by(|a, b| a.filename.cmp(&b.filename));
        Ok(db)
    }

    pub fn skip_login(&self) -> bool {
        self.skip_login
    }

    pub fn skip_login_set(&mut self, v: bool) {
        self.skip_login = v;
    }

    pub fn anilist_access_token(&self) -> Option<&str> {
        self.anilist_cred.as_ref().map(|v| v.access_token())
    }

    pub fn anilist_cred_set(&mut self, cred: Option<AniListCred>) {
        self.anilist_cred = cred;
    }

    pub fn anilist_cred(&self) -> &Option<AniListCred> {
        &self.anilist_cred
    }

    pub fn anilist_user_id(&self) -> Option<u64> {
        self.anilist_cred.as_ref().map(|v| v.user_id())
    }

    pub fn anilist_clear(&mut self) {
        self.anilist_cred = None;
    }

    pub fn len(&self) -> usize {
        self.cached_view.animes.len()
    }

    pub fn retrieve_images(&mut self, image_directory: &str) -> anyhow::Result<()> {
        if !Path::new(image_directory).exists() {
            std::fs::create_dir(image_directory)?;
        }
        for anime in &mut self.anime_map {
            if let Some(metadata) = &anime.metadata {
                let thumbnail_path = format!("{image_directory}/{}.jpg", metadata.title());
                if !std::path::Path::new(&thumbnail_path).exists() {
                    download_image(metadata.thumbnail(), &thumbnail_path)?;
                }
                anime.thumbnail = Some(thumbnail_path);
            }
        }
        Ok(())
    }

    pub fn update_directory(&mut self, directory: impl AsRef<str>, time: u64, buf: &mut String) {
        let mut sanitized_name = buf;
        self.previous_update.push((directory.as_ref().into(), time));

        read_dir(directory.as_ref())
            .unwrap()
            .filter_map(|v| v.ok())
            .filter(|v| !is_empty_dir_entry(v))
            .map(|v| (o_to_str!(v.file_name()), v.path()))
            .for_each(|(name, path)| {
                match self
                    .anime_map
                    .iter_mut()
                    .find(|anime| anime.filename == name)
                {
                    None => {
                        let mut chars = name.chars();
                        sanitize::sanitize_name(&mut chars, &mut sanitized_name);

                        // `JsonIndexed::map()` calls to `optimize_json_db`, which
                        // does not invalidate any references.
                        //
                        // Unsafe used to get around lifetime restrictions.
                        let indexed_db = self.indexed_db.get_or_insert_with(JsonIndexed::new);
                        let indexed_json: &'a mut JsonIndexed =
                            unsafe { &mut *(indexed_db as *mut _) };
                        let map = indexed_json.map();
                        let metadata = self
                            .indexed_db
                            .get_or_insert_with(JsonIndexed::new)
                            .match_name(map, sanitized_name.trim());
                        self.anime_map
                            .push(Anime::from_path(path, name, metadata.cloned(), time));
                        sanitized_name.clear();
                    }
                    Some(v) => {
                        if v.last_updated < dir_modified_time(&path).unwrap()
                            || v.episodes().first().is_some_and(|(_, v)| {
                                !v.iter().any(|v| {
                                    Path::new(v).canonicalize().is_ok_and(|v| {
                                        v.parent().unwrap().eq(&path.canonicalize().unwrap())
                                    })
                                })
                            })
                        {
                            v.update_episodes();
                        }
                    }
                };
            });
    }

    pub fn update(&mut self, anime_directories: Vec<impl AsRef<str>>) {
        let time = get_time();
        let mut sanitized_name = String::with_capacity(64);

        for directory in anime_directories {
            if Path::new(directory.as_ref()).exists() {
                self.update_directory(directory, time, &mut sanitized_name);
            }
        }
    }

    pub fn update_watched(&mut self, anime: &mut Anime, episode: Episode) -> Result<()> {
        anime.update_watched(episode)?;
        anime.update_episodes();
        Ok(())
    }

    pub fn update_cached(&mut self) {
        // Unsafe is needed as `cached_view` takes mutable references to
        // `anime_map`.
        //
        // This unties the lifetime of `anime_map` from `self` and allows
        // multiple mutable references.
        //
        // Doing this will not cause invariances unless `anime_map` is
        // mutated (such as inserting or removing), *so don't do that*.

        // TODO: cached_view use indices
        let anime_map: &'a mut Vec<Anime> = unsafe { &mut *(&mut self.anime_map as *mut _) };
        self.cached_view.last_updated = get_time();
        self.cached_view.animes = anime_map
            .iter_mut()
            .filter(|v| v.paths.iter().any(|v| Path::new(&v).exists()))
            .collect();
        self.cached_view
            .animes
            .sort_by(|a, b| b.last_watched.cmp(&a.last_watched));
    }

    pub fn write(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let mut f = File::create(path)?;
        let mut s = flexbuffers::FlexbufferSerializer::new();
        self.serialize(&mut s)?;
        f.write_all(s.view())?;
        Ok(())
    }

    pub fn animes<'frame>(&mut self) -> &'frame mut [&'frame mut Anime] {
        if self
            .cached_view
            .animes
            .iter()
            .any(|v| v.last_watched > self.cached_view.last_updated)
        {
            self.update_cached();
        }

        // Unsafe is needed to untie the lifetime of `cached_view` from `self`.
        //
        // References to this slice is only expected to live for one frame, and
        // `cached_view` should not be mutated while being borrowed.
        unsafe { std::mem::transmute(self.cached_view.animes.as_mut_slice()) }
    }

    pub fn cache_idx_to_map_idx(&self, idx: usize) -> usize {
        let anime = &self.cached_view.animes[idx];
        self.anime_map
            .iter()
            .enumerate()
            .find(|(_, v)| (*v) as *const _ == (*anime) as *const _)
            .unwrap()
            .0
    }

    pub fn get_idx(&self, idx: usize) -> &Anime {
        &self.anime_map[idx]
    }

    pub fn get_mut_idx(&mut self, idx: usize) -> &mut Anime {
        &mut self.anime_map[idx]
    }

    pub fn get_anime<'b>(&mut self, anime: impl AsRef<str>) -> Option<&'b mut Anime> {
        self.anime_map
            .iter_mut()
            .find(|v| v.filename == anime.as_ref())
            .map(|v| unsafe { &mut *(v as *mut _) })
    }

    pub fn fuzzy_find_anime(&mut self, input: &str) -> Box<[&'a AnimeDatabaseData]> {
        self.indexed_db
            .get_or_insert_with(JsonIndexed::new)
            .fuzzy_find_anime(input)
    }

    pub fn update_media<'b>(&mut self, entry: &MediaEntry) -> Vec<&'b mut Anime> {
        let mut vec = vec![];
        'anime: for anime in self.animes() {
            match anime.anilist_id() {
                Some(anilist_id) if anilist_id == entry.id() => {
                    if anime.last_watched > entry.updated_at() {
                        let anime = unsafe { &mut *(*anime as *mut Anime) };
                        vec.push(anime);
                        continue 'anime;
                    }

                    anime.set_last_watched(entry.updated_at());
                    anime.set_progress(entry.progress());
                }
                _ => (),
            }
        }
        vec
    }

    /// Returns list of entries that need to be updated
    pub fn update_anilist_list<'b>(&mut self, collection: &Collection) -> Box<[&'b mut Anime]> {
        collection
            .entries()
            .iter()
            .flat_map(|entry| self.update_media(entry).into_iter())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, path::PathBuf};

    use super::is_empty_dir;

    #[test]
    fn btree_test() {
        let btree = [("hello", 20), ("hi", 5), ("hello", 1)].into_iter().fold(
            BTreeMap::new(),
            |mut acc, (k, v)| {
                acc.entry(k)
                    .and_modify(|list: &mut Vec<usize>| list.push(v))
                    .or_insert(vec![v]);
                acc
            },
        );
        assert_eq!(
            BTreeMap::from([("hello", vec![20, 1]), ("hi", vec![5])]),
            btree
        );
    }

    #[test]
    fn empty_directory_test() {
        let directory = PathBuf::from("tests/empty-dir-test");
        std::fs::create_dir_all(&directory).unwrap();
        assert!(directory.exists());
        assert!(is_empty_dir(directory));
        //read_dir
    }
}
