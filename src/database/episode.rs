use std::{fmt::Display, path::Path, str::FromStr};

use regex::Regex;
use serde::{Serialize, Deserialize};
use thiserror::Error;
lazy_static::lazy_static! {
    static ref REG_EPS: Regex = Regex::new(r#"(?:(?:^|S|s)(?P<s>\d{2}))?(?: )?(?:_|x|E|e|EP|ep| )(?P<e>\d{1,2})(?:.bits|_| |-|\.|v|$)"#).unwrap();
    static ref REG_PARSE_OUT: Regex = Regex::new(r#"(x256|x265|\d{4}|\d{3})|10.bits"#).unwrap();
    static ref REG_SPECIAL: Regex =
    Regex::new(r#".*OVA.*\.|NCED.*? |NCOP.*? |(-|_| )(ED|OP|SP|no-credit_opening|no-credit_ending).*?(-|_| )"#).unwrap();
}

#[derive(Debug, PartialEq, Ord, Eq, Clone, Deserialize, Serialize)]
pub enum Episode {
    Numbered { season: u32, episode: u32 },
    Special { filename: String },
}

impl Display for Episode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Numbered { season, episode } => write!(f, "S{season:02} E{episode:02}"),
            Self::Special { filename } => filename.fmt(f),
        }
    }
}

impl From<(u32, u32)> for Episode {
    fn from((season, episode): (u32, u32)) -> Self {
        Self::Numbered { season, episode }
    }
}

impl PartialOrd for Episode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self {
            Self::Numbered {
                season: season_a,
                episode: episode_a,
                ..
            } => match other {
                Self::Numbered {
                    season: season_b,
                    episode: episode_b,
                    ..
                } => {
                    if season_a == season_b {
                        Some(episode_a.cmp(episode_b))
                    } else {
                        Some(season_a.cmp(season_b))
                    }
                }
                Self::Special { .. } => Some(std::cmp::Ordering::Greater),
            },
            Self::Special {
                filename: filename_a,
                ..
            } => match other {
                Self::Numbered { .. } => Some(std::cmp::Ordering::Less),
                Self::Special {
                    filename: filename_b,
                    ..
                } => Some(filename_a.cmp(filename_b)),
            },
        }
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum EpisodeParseError {
    #[error("Invalid path to episode")]
    InvalidFile,
    #[error("Unable to convert file to UTF-8 string")]
    UTF8,
    #[error("Invalid episode format")]
    InvalidFormat(String),
}

impl FromStr for Episode {
    type Err = EpisodeParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if REG_SPECIAL.is_match(s) {
            return Ok(Self::Special {
                filename: s.to_owned(),
            });
        }

        match REG_EPS.captures(&REG_PARSE_OUT.replace_all(s, "#")) {
            Some(caps) => {
                let season = caps
                    .name("s")
                    .map(|a| a.as_str().parse().expect("Capture is integer"))
                    .unwrap_or(1);
                let episode = caps
                    .name("e")
                    .map(|a| a.as_str().parse().expect("Capture is integer"))
                    .ok_or_else(|| Self::Err::InvalidFormat(s.to_string()))?;
                return Ok(Self::Numbered { season, episode });
            }
            None => {
                return Ok(Self::Special {
                    filename: s.to_string(),
                })
            }
        }
    }
}

impl TryFrom<&Path> for Episode {
    type Error = EpisodeParseError;

    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        let filename = path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
        Ok(filename.parse()?)
    }
}

impl Episode {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Episode, EpisodeParseError> {
        Episode::try_from(path.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn episode_sort_0() {
        let a = Episode::Numbered {
            season: 1,
            episode: 1,
        };
        let b = Episode::Numbered {
            season: 1,
            episode: 2,
        };
        assert!(a < b);
    }

    #[test]
    fn episode_sort_1() {
        let a = Episode::Numbered {
            season: 1,
            episode: 2,
        };
        let b = Episode::Numbered {
            season: 2,
            episode: 1,
        };
        assert!(a < b);
    }

    #[test]
    fn episode_sort_2() {
        let a = Episode::Special {
            filename: String::from("abc"),
        };
        let b = Episode::Numbered {
            season: 2,
            episode: 1,
        };
        assert!(a < b);
    }

    #[test]
    fn episode_sort_3() {
        let a = Episode::Numbered {
            season: 2,
            episode: 1,
        };
        let b = Episode::Special {
            filename: String::from("abc"),
        };
        assert!(a > b);
    }

    #[test]
    fn episode_from_str_0() {
        let filename = r"[sam] Vinland Saga - 24 [BD 1080p FLAC] [6696F95B].mkv".to_string();
        assert_eq!(
            Ok(Episode::Numbered {
                season: 1,
                episode: 24,
            }),
            Episode::from_str(&filename)
        );
    }

    #[test]
    fn episode_from_str_1() {
        let filename =
            r"Girls.und.Panzer.S01E04.1080p-Hi10p.BluRay.FLAC2.1.x264-CTR.[1123C40D].mkv"
                .to_string();
        assert_eq!(
            Ok(Episode::Numbered {
                season: 1,
                episode: 4,
            }),
            Episode::from_str(&filename)
        );
    }

    #[test]
    fn episode_from_str_2() {
        let filepath = Path::new(r"[Datte13] Yuyushiki - S01E12 - Uneventful Good Life.mkv");
        assert_eq!(
            Ok(Episode::Numbered {
                season: 1,
                episode: 12,
            }),
            Episode::try_from(filepath)
        );
    }

    #[test]
    fn episode_from_str_3() {
        let filename = r"[Arid] Sound! Euphonium - Creditless OP [D04F5D1D].mkv".to_string();
        assert_eq!(
            Ok(Episode::Special {
                filename: filename.clone()
            }),
            Episode::from_str(&filename)
        );
    }

    #[test]
    fn episode_from_str_4() {
        let filepath = Path::new(r"Girls.und.Panzer.S00E02.Water.War!.4.6.1080p-Hi10p.BluRay.FLAC2.1.x264-CTR.[ABDE84A3].mkv");
        assert_eq!(
            Ok(Episode::Numbered {
                season: 0,
                episode: 2,
            }),
            Episode::try_from(filepath)
        );
    }

    #[test]
    fn episode_from_str_5() {
        let s = "S00 E03";
        assert_eq!(
            Ok(Episode::Numbered {
                season: 0,
                episode: 3,
            }),
            Episode::from_str(s)
        );
    }
}
