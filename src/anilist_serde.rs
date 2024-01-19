use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub enum Viewer {
    Ok(u64),
    Err(ViewerError),
}

#[derive(Clone, Debug)]
pub enum MediaList {
    Ok(Box<[Collection]>),
    Err(MediaListError),
}

impl Viewer {
    pub fn deserialize_json(bytes: &[u8]) -> anyhow::Result<Self> {
        match serde_json::from_slice::<__Viewer>(bytes)? {
            __Viewer::Ok { data } => match data {
                ViewerData::Viewer { id } => Ok(Self::Ok(id)),
            },
            __Viewer::Err(e) => Ok(Self::Err(e)),
        }
    }
}

impl MediaList {
    pub fn deserialize_json(bytes: &[u8]) -> anyhow::Result<Self> {
        match serde_json::from_slice::<__MediaList>(bytes)? {
            __MediaList::Ok { data } => Ok(Self::Ok(data.media_list_collection.lists)),
            __MediaList::Err(e) => Ok(Self::Err(e)),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum __Viewer {
    Ok { data: ViewerData },
    Err(ViewerError),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum __MediaList {
    Ok { data: MediaListCollection },
    Err(MediaListError),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct MediaListCollection {
    #[serde(rename = "MediaListCollection")]
    media_list_collection: MediaCollectionData,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct MediaCollectionData {
    lists: Box<[Collection]>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Collection {
    name: String,
    status: String,
    entries: Box<[MediaEntry]>,
}

impl Collection {
    pub fn entries(&self) -> &[MediaEntry] {
        &self.entries
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn status(&self) -> &str {
        &self.status
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaEntry {
    progress: u32,
    #[serde(rename = "updatedAt")]
    updated_at: u64,
    media: Media,
}

impl MediaEntry {
    pub fn progress(&self) -> u32 {
        self.progress
    }

    pub fn updated_at(&self) -> u64 {
        self.updated_at
    }

    pub fn id(&self) -> u32 {
        self.media.id
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Media {
    id: u32,
}

// TODO: Proper errors
//
// Notes: Will typically be an incorrect token error
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ViewerError {}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaListError {}

#[derive(Debug, Serialize, Deserialize, Clone)]
enum ViewerData {
    Viewer { id: u64 },
}
