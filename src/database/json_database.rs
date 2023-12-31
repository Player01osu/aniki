use super::sanitize::sanitize_name;
use crate::database::Database;
use fuzzy_matcher::skim::{SkimMatcherV2, SkimScoreConfig};
use fuzzy_matcher::FuzzyMatcher;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::path::Path;

const JSON_RAW: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/anime-offline-database.json"));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimeSeason {
    season: String,
    year: Option<u32>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimeDatabaseData {
    pub sources: Vec<String>,
    pub title: String,
    pub synonyms: Vec<String>,
    #[serde(rename = "picture")]
    pub thumbnail: String,
    pub tags: Vec<String>,
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
    data: Vec<AnimeDatabaseData>,
}

//#[derive(Debug)]
pub struct OptimizedDatabase {
    pub map: BTreeMap<(char, char, char), Vec<AnimeDatabaseData>>,
    pub search_map: Option<BTreeMap<(char, char, char), BTreeSet<String>>>,
}

fn format_string(s: &str, f: &mut std::fmt::Formatter<'_>) {
    write!(f, r####"String::from(r###"{s}"###)"####).ok();
}

impl std::fmt::Debug for AnimeDatabaseData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, r#"AnimeDatabaseData{{"#).ok();

        write!(f, r#"title:"#).ok();
        format_string(&self.title, f);
        write!(f, r#",thumbnail:"#).ok();
        format_string(&self.thumbnail, f);
        write!(f, r#",sources:"#).ok();
        format_vec_string(&self.sources, f);
        write!(f, r#",synonyms:"#).ok();
        format_vec_string(&self.synonyms, f);
        write!(f, r#",tags:"#).ok();
        format_vec_string(&self.tags, f);
        write!(f, r#"}}"#).ok();

        Ok(())
    }
}

fn format_vec_string(vec: &[String], f: &mut std::fmt::Formatter<'_>) {
    write!(f, "vec![").ok();
    for (i, a) in vec.iter().enumerate() {
        if i != 0 {
            write!(f, ",").ok();
        }
        format_string(a, f);
    }
    write!(f, "]").ok();
}

fn format_vec(vec: &[impl std::fmt::Debug], f: &mut std::fmt::Formatter<'_>) {
    write!(f, "vec![").ok();
    for (i, a) in vec.iter().enumerate() {
        if i != 0 {
            write!(f, ",").ok();
        }
        write!(f, "{a:?}").ok();
    }
    write!(f, "]").ok();
}

fn format_kv(
    map: &BTreeMap<(char, char, char), Vec<AnimeDatabaseData>>,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    for ((c1, c2, c3), v) in map.iter() {
        write!(f, r"(({c1:?},{c2:?},{c3:?}),").ok();
        format_vec(v, f);
        write!(f, r")").ok();
    }
    Ok(())
}

impl std::fmt::Debug for OptimizedDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, r#"OptimizedDatabase{{map:BTreeMap::from(["#).ok();
        format_kv(&self.map, f).ok();
        write!(f, r#"])}}"#)
        //write!(
        //    f,
        //    r#"OptimizedDatabase{{map:BTreeMap::from([{}])}}"#,
        //    format_kv(&self.map, f)
        //)
    }
}

pub fn open_json_db(_path: impl AsRef<Path>) -> AnimeDatabaseJson {
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
fn skim_matcher() -> SkimMatcherV2 {
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
pub fn sanitize_cache_name() -> Vec<String> {
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
    sanitized_names
}

fn str_idx(s: &str) -> (char, char, char) {
    let mut chars = s.chars();
    (
        chars.next().unwrap().to_ascii_uppercase(),
        chars.next().unwrap_or('\0').to_ascii_uppercase(),
        chars.next().unwrap_or('\0').to_ascii_uppercase(),
    )
}

impl OptimizedDatabase {
    pub fn find(&self, key: &str) -> AnimeDatabaseData {
        let (i, j, k) = str_idx(key);
        self.map
            .get(&(i, j, k))
            .unwrap()
            .iter()
            .find(|v| v.title == key || v.synonyms.iter().any(|title| title == key))
            .unwrap()
            .clone()
    }

    pub fn match_names(&self, sanitized_names: &[String]) -> Vec<Option<AnimeDatabaseData>> {
        let matcher = skim_matcher();
        let mut name_matches = Vec::with_capacity(sanitized_names.len());
        let mut name_heap = BinaryHeap::new();
        for name in sanitized_names {
            let (i, j, k) = str_idx(name);
            let map = match self.map.get(&(i, j, k)) {
                Some(m) => m,
                None => continue,
            };
            for anime in map.iter() {
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
                Some((_, k)) => name_matches.push(Some(self.find(k))),
                None => name_matches.push(None),
            }
            name_heap.clear();
        }
        name_matches
    }

    pub fn match_name(&self, sanitized_name: &str) -> Option<AnimeDatabaseData> {
        let matcher = skim_matcher();
        let mut name_heap = BinaryHeap::new();
        let (i, j, k) = str_idx(sanitized_name);
        let map = match self.map.get(&(i, j, k)) {
            Some(m) => m,
            None => return None,
        };
        for anime in map.iter() {
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
        name_heap.pop().map(|(_, k)| self.find(k))
    }

    pub fn optimize_json_db(json_database: AnimeDatabaseJson) -> Self {
        let mut optimized = Self {
            map: BTreeMap::new(),
            search_map: None,
        };

        for anime in json_database.data.into_iter() {
            {
                // Title
                let mut chars = anime.title.chars();
                let c = chars.next().unwrap();
                let c2 = chars.next().unwrap_or('\0');
                let c3 = chars.next().unwrap_or('\0');
                if c_filter(c) && c_filter(c2) && c_filter(c3) {
                    match optimized.map.get_mut(&(c_idx!(c), c_idx!(c2), c_idx!(c3))) {
                        Some(v) => v.push(anime.clone()),
                        None => {
                            optimized
                                .map
                                .insert((c_idx!(c), c_idx!(c2), c_idx!(c3)), Vec::new());
                        }
                    };
                }
            }

            for name in anime.synonyms.iter() {
                let mut chars = name.chars();
                let c = chars.next().unwrap();
                let c2 = chars.next().unwrap_or('\0');
                let c3 = chars.next().unwrap_or('\0');
                if c_filter(c) && c_filter(c2) && c_filter(c3) {
                    match optimized.map.get_mut(&(c_idx!(c), c_idx!(c2), c_idx!(c3))) {
                        Some(v) => v.push(anime.clone()),
                        None => {
                            optimized
                                .map
                                .insert((c_idx!(c), c_idx!(c2), c_idx!(c3)), Vec::new());
                        }
                    };
                }
            }
        }
        optimized
    }

    pub fn optimize_json_db_search(
        optimized: OptimizedDatabase,
        json_database: AnimeDatabaseJson,
    ) -> OptimizedDatabase {
        let mut optimized = OptimizedDatabase {
            map: optimized.map,
            search_map: None,
        };
        let mut map = BTreeMap::<(char, char, char), BTreeSet<String>>::new();

        for anime in json_database.data.into_iter() {
            for token in tokenize(&anime.title).iter() {
                // Title
                let mut chars = token.chars();
                let c = chars.next().unwrap();
                let c2 = chars.next().unwrap_or('\0');
                let c3 = chars.next().unwrap_or('\0');
                if c_filter(c) && c_filter(c2) && c_filter(c3) {
                    match map.get_mut(&(c_idx!(c), c_idx!(c2), c_idx!(c3))) {
                        Some(v) => {
                            v.insert(anime.title.clone());
                        }
                        None => {
                            map.insert((c_idx!(c), c_idx!(c2), c_idx!(c3)), BTreeSet::new());
                        }
                    };
                }
            }

            for name in anime.synonyms.iter() {
                for token in tokenize(&name).iter() {
                    let mut chars = token.chars();
                    let c = chars.next().unwrap();
                    let c2 = chars.next().unwrap_or('\0');
                    let c3 = chars.next().unwrap_or('\0');
                    if c_filter(c) && c_filter(c2) && c_filter(c3) {
                        match map.get_mut(&(c_idx!(c), c_idx!(c2), c_idx!(c3))) {
                            Some(v) => {
                                v.insert(name.clone());
                            }
                            None => {
                                map.insert((c_idx!(c), c_idx!(c2), c_idx!(c3)), BTreeSet::new());
                            }
                        };
                    }
                }
            }
        }
        optimized.search_map = Some(map);
        optimized
    }

}
