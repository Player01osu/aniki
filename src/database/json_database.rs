use super::sanitize::sanitize_name;
use crate::database::Database;
use fuzzy_matcher::skim::{SkimMatcherV2, SkimScoreConfig};
use fuzzy_matcher::FuzzyMatcher;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BinaryHeap};
use std::path::Path;
use std::str::Chars;

const JSON_RAW: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/anime-offline-database.json"));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimeSeason {
    season: String,
    year: Option<u32>,
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimeDatabaseData {
    pub sources: Box<[String]>,
    pub title: String,
    pub synonyms: Box<[String]>,
    #[serde(rename = "picture")]
    pub thumbnail: String,
    pub tags: Box<[String]>,
}

impl AnimeDatabaseData {
    pub fn sources(&self) -> &[String] {
        &self.sources
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn synonyms(&self) -> &[String] {
        &self.synonyms
    }
    pub fn thumbnail(&self) -> &str {
        &self.thumbnail
    }
    pub fn tags(&self) -> &[String] {
        &self.tags
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimeDatabaseJson {
    last_update: String,
    data: Box<[AnimeDatabaseData]>,
}

pub type OptimizedMap<'a> = BTreeMap<OptimizedKey, OptimizedValue<'a>>;
type OptimizedKey = (char, char, char);
type OptimizedValue<'a> = Vec<&'a AnimeDatabaseData>;

#[derive(Debug)]
struct OptimizedDatabase<'a> {
    map: Option<BTreeMap<OptimizedKey, OptimizedValue<'a>>>,
    search_map: Option<BTreeMap<OptimizedKey, OptimizedValue<'a>>>,
}

#[derive(Debug)]
pub struct JsonIndexed<'a> {
    optimized: OptimizedDatabase<'a>,
    json_database: AnimeDatabaseJson,
}

impl Default for JsonIndexed<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'b> JsonIndexed<'b> {
    pub fn new() -> Self {
        let json_database = open_json_db("");
        let optimized = OptimizedDatabase::new();
        Self {
            optimized,
            json_database,
        }
    }

    pub fn find<'a>(&self, map: &'a OptimizedMap, key: &str) -> &'a AnimeDatabaseData {
        let (i, j, k) = str_idx(key);
        map.get(&(i, j, k))
            .unwrap()
            .iter()
            .find(|v| v.title == key || v.synonyms.iter().any(|title| title == key))
            .unwrap()
    }

    pub fn match_names<'a>(
        &self,
        map: &'a OptimizedMap,
        sanitized_names: &[String],
    ) -> Box<[Option<&'a AnimeDatabaseData>]> {
        let matcher = skim_matcher();
        let mut name_matches = Vec::with_capacity(sanitized_names.len());
        let mut name_heap = BinaryHeap::new();
        for name in sanitized_names {
            let (i, j, k) = str_idx(name);
            let set = match map.get(&(i, j, k)) {
                Some(m) => m,
                None => continue,
            };
            for anime in set.iter() {
                let title = &anime.title;
                if let Some(weight) = matcher.fuzzy_match(name, title) {
                    name_heap.push((weight, title));
                }

                for synonym in anime.synonyms.iter() {
                    if let Some(weight) = matcher.fuzzy_match(name, synonym) {
                        name_heap.push((weight, synonym));
                    }
                }
            }
            match name_heap.pop() {
                Some((_, k)) => name_matches.push(Some(self.find(map, k))),
                None => name_matches.push(None),
            }
            name_heap.clear();
        }
        name_matches.into()
    }

    pub fn match_name<'a>(
        &self,
        map: &'a OptimizedMap,
        sanitized_name: &str,
    ) -> Option<&'a AnimeDatabaseData> {
        let matcher = skim_matcher();
        let mut name_heap = BinaryHeap::new();
        let (i, j, k) = str_idx(sanitized_name);
        let set = match map.get(&(i, j, k)) {
            Some(m) => m,
            None => return None,
        };
        for anime in set.iter() {
            let title = &anime.title;
            if let Some(weight) = matcher.fuzzy_match(sanitized_name, title) {
                name_heap.push((weight, title));
            }

            for synonym in anime.synonyms.iter() {
                if let Some(weight) = matcher.fuzzy_match(sanitized_name, synonym) {
                    name_heap.push((weight, synonym));
                }
            }
        }
        name_heap.pop().map(|(_, k)| self.find(map, k))
    }

    pub fn map(&mut self) -> &'b OptimizedMap {
        self.optimized.optimize_json_db(&self.json_database)
    }

    pub fn search_map(&mut self) -> &'b OptimizedMap {
        self.optimized.optimize_json_db_search(&self.json_database)
    }
}

fn open_json_db(_path: impl AsRef<Path>) -> AnimeDatabaseJson {
    serde_json::from_slice(JSON_RAW).unwrap()
}

macro_rules! c_idx {
    ($c: expr) => {
        $c.to_ascii_uppercase()
    };
}

#[inline]
fn c_filter(c: char) -> bool {
    c.is_ascii()
}

#[inline]
pub fn skim_matcher() -> SkimMatcherV2 {
    let score_match = 16;
    let gap_start = -3;
    let gap_extension = -1;
    let bonus_first_char_multiplier = 3;

    let score_cfg = SkimScoreConfig {
        score_match,
        gap_start,
        gap_extension,
        bonus_first_char_multiplier,
        bonus_head: score_match * 2 / 3,
        bonus_break: score_match / 2 + gap_extension,
        bonus_camel: score_match / 2 + 2 * gap_extension,
        bonus_consecutive: -(gap_start + gap_extension),
        penalty_case_mismatch: gap_extension * 2,
    };

    SkimMatcherV2::default()
        .score_config(score_cfg)
        .ignore_case()
}

fn tokenize(s: &str) -> Box<[&str]> {
    s.split_whitespace().collect()
}
pub fn sanitize_cache_name() -> Box<[String]> {
    let mut database =
        Database::new("./anime-cache.db", vec!["/home/bruh/Videos/not-anime"]).unwrap();
    let animes = database.animes();
    let mut sanitized_names = vec![];
    for anime in animes.iter() {
        let mut chars = anime.filename.chars();
        let mut buf = String::new();
        sanitize_name(&mut chars, &mut buf);
        sanitized_names.push(buf.trim().to_string());
    }
    sanitized_names.into()
}

fn str_idx(s: &str) -> (char, char, char) {
    let mut chars = s.chars();
    (
        chars.next().unwrap().to_ascii_uppercase(),
        chars.next().unwrap_or('\0').to_ascii_uppercase(),
        chars.next().unwrap_or('\0').to_ascii_uppercase(),
    )
}

fn insert_index_map<'a>(
    map: &mut OptimizedMap<'a>,
    mut chars: Chars,
    anime: &'a AnimeDatabaseData,
) {
    let c = chars.next().unwrap();
    let c2 = chars.next().unwrap_or('\0');
    let c3 = chars.next().unwrap_or('\0');
    if c_filter(c) && c_filter(c2) && c_filter(c3) {
        match map.get_mut(&(c_idx!(c), c_idx!(c2), c_idx!(c3))) {
            Some(v) => {
                v.push(anime);
            }
            None => {
                map.insert((c_idx!(c), c_idx!(c2), c_idx!(c3)), Vec::new());
            }
        };
    }
}

impl<'a> OptimizedDatabase<'a> {
    fn new() -> Self {
        Self {
            map: None,
            search_map: None,
        }
    }

    fn optimize_json_db(&mut self, json_database: &'a AnimeDatabaseJson) -> &'a OptimizedMap {
        // `OptimizedMap` has references to `json_database`, but this is only used in the context
        // of `JsonIndexed` which has an owned _immutable_ reference to `json_database`.
        //
        // Using an unsafe ptr cast here gets around borrowing restrictions.
        let json_database: &'a AnimeDatabaseJson = unsafe { &*(json_database as *const _) };
        self.map.get_or_insert_with(|| {
            let mut map = BTreeMap::<OptimizedKey, OptimizedValue>::new();

            for anime in json_database.data.iter() {
                for name in anime.synonyms.iter().chain([&anime.title]) {
                    let chars = name.chars();
                    insert_index_map(&mut map, chars, anime);
                }
            }
            map
        })
    }

    fn optimize_json_db_search(
        &mut self,
        json_database: &'a AnimeDatabaseJson,
    ) -> &'a OptimizedMap {
        // See comments about unsafe use in `optimized_json_db`.
        let json_database: &'a AnimeDatabaseJson = unsafe { &*(json_database as *const _) };
        self.search_map.get_or_insert_with(|| {
            let mut map = BTreeMap::<OptimizedKey, OptimizedValue>::new();

            for anime in json_database.data.iter() {
                for name in anime.synonyms.iter().chain([&anime.title]) {
                    for token in tokenize(&name).iter() {
                        let chars = token.chars();
                        insert_index_map(&mut map, chars, anime);
                    }
                }
            }
            map
        })
    }
}
