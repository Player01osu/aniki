use std::{sync::mpsc, time::Duration};

use crate::{
    anilist_serde::{MediaEntry, MediaList, Viewer},
    database::{self, AniListCred},
    ui::update_anilist_watched,
    App, ConnectionOverlayState, LoginProgress, CONNECTION_OVERLAY_TIMEOUT,
};

pub type HttpSender = mpsc::Sender<anyhow::Result<HttpData>>;

pub fn get_anilist_media_list(tx: &HttpSender, user_id: u64, access_token: &str) {
    let access_token = access_token.to_string();
    send_request(
        tx,
        RequestKind::GetAnilistMediaList {
            user_id,
            access_token,
        },
    );
}

pub fn send_login(tx: &HttpSender, access_token: &str) {
    let access_token = access_token.to_string();
    send_request(tx, RequestKind::SendLogin { access_token });
}

#[derive(Debug)]
pub enum RequestKind {
    GetAnilistMediaList {
        access_token: String,
        user_id: u64,
    },
    SendLogin {
        access_token: String,
    },
    UpdateMedia {
        access_token: String,
        media_id: u32,
        episode: u32,
        ptr_id: u64,
    },
    Test(String),
}

#[derive(Clone, Debug)]
pub enum HttpData {
    Viewer(Viewer, String),
    MediaList(MediaList),
    UpdateMedia(u64 /* ptr_id */, MediaEntry),
    Debug(String),
}

pub fn send_request(tx: &mpsc::Sender<anyhow::Result<HttpData>>, request: RequestKind) {
    let tx = tx.clone();
    tokio::spawn(async move {
        let request_err = |err| eprintln!("ERROR:failed to await request:{err}");
        let byte_err = |err| eprintln!("ERROR:failed to receive bytes:{err}");
        let deserialize_err = |err| println!("ERROR:failed to deserialize json:{err}");
        let send_err = |err| eprintln!("ERROR:failed to send http data:{err}");

        match request {
            RequestKind::GetAnilistMediaList {
                user_id,
                access_token,
            } => {
                let anime_list_query = include_str!("anime_list.gql");
                let json = serde_json::json!({"query": anime_list_query, "variables": {"id": 15125, "uid": user_id}});
                let res = reqwest::Client::new()
                    .post("https://graphql.anilist.co")
                    .header("Authorization", format!("Bearer {access_token}"))
                    .header("Content-Type", "application/json")
                    .header("Accept", "application/json")
                    .body(json.to_string())
                    .timeout(Duration::from_secs(10))
                    .send()
                    .await
                    .map_err(request_err)?;
                tx.send(Ok(HttpData::MediaList(
                    MediaList::deserialize_json(&res.bytes().await.map_err(byte_err)?)
                        .map_err(deserialize_err)?,
                )))
                .map_err(send_err)
            }
            RequestKind::SendLogin { access_token } => {
                let mutation = r#"query { Viewer { id } }"#;
                let json = serde_json::json!({"query": mutation, "variables": {"id": 15125}});
                let res = reqwest::Client::new()
                    .post("https://graphql.anilist.co")
                    .header("Authorization", format!("Bearer {access_token}"))
                    .header("Content-Type", "application/json")
                    .header("Accept", "application/json")
                    .body(json.to_string())
                    .timeout(Duration::from_secs(10))
                    .send()
                    .await
                    .map_err(request_err)?;
                let data = HttpData::Viewer(
                    Viewer::deserialize_json(&res.bytes().await.map_err(byte_err)?)
                        .map_err(deserialize_err)?,
                    access_token.to_string(),
                );
                tx.send(Ok(data)).map_err(send_err)
            }
            // TODO: If the same episode is pressed, the `last_watched` is updateed
            // locally, but the `updatedAt` is not updated in anilist.
            //
            // This causes unnecessary requests to anilist everytime the aniki is opened.
            //
            // Maybe have a dirty bit on every anime entry that is cleared by this?
            RequestKind::UpdateMedia {
                access_token,
                media_id,
                episode,
                ptr_id,
            } => {
                let anime_list_query = include_str!("update_anilist_media.gql");
                let json = serde_json::json!({"query": anime_list_query, "variables": {"id": 15125, "mediaId": media_id, "episode": episode}});
                let res = reqwest::Client::new()
                    .post("https://graphql.anilist.co")
                    .header("Authorization", format!("Bearer {access_token}"))
                    .header("Content-Type", "application/json")
                    .header("Accept", "application/json")
                    .body(json.to_string())
                    .timeout(Duration::from_secs(10))
                    .send()
                    .await
                    .map_err(request_err)?;
                let data = HttpData::UpdateMedia(
                    ptr_id,
                    MediaEntry::deserialize_json(&res.bytes().await.map_err(byte_err)?)
                        .map_err(deserialize_err)?,
                );
                tx.send(anyhow::Ok(data))
                    .map_err(|err| eprintln!("ERROR:failed to send http data:{err}"))
            }
            RequestKind::Test(s) => {
                eprintln!("Got test:{s}");
                anyhow::Result::Ok(())
            }
        }
    });
}

fn sync_to_anilist(tx: &HttpSender, access_token: &str, animes: &mut [&mut database::Anime]) {
    for anime in animes {
        update_anilist_watched(tx, access_token, *anime);
    }
}

pub fn poll_http(app: &mut App) {
    if let Ok(data) = app.http_rx.try_recv() {
        let data = match data {
            Ok(v) => v,
            Err(e) => {
                eprintln!("ERROR:Something went wrong in http thread:{e}");
                return;
            }
        };

        match data {
            HttpData::Viewer(viewer, access_token) => match viewer {
                Viewer::Ok(id) => {
                    app.database
                        .anilist_cred_set(Some(AniListCred::new(id, access_token)));
                    app.login_progress = LoginProgress::None;
                    app.connection_overlay.state = ConnectionOverlayState::Connected;
                    app.connection_overlay.timeout = CONNECTION_OVERLAY_TIMEOUT;
                }
                Viewer::Err(_) => {
                    app.login_progress = LoginProgress::Failed;
                    app.login_state.textbox.text.clear();
                }
            },
            HttpData::MediaList(media_list) => match media_list {
                MediaList::Ok(collections) => {
                    for collection in collections.iter() {
                        let mut sync_newer = app.database.update_anilist_list(collection);

                        if let Some(access_token) = app.database.anilist_access_token() {
                            sync_to_anilist(&app.http_tx, access_token, &mut sync_newer);
                        }
                    }
                    app.database.update_cached();
                }
                MediaList::Err(_) => {
                    eprintln!("{}:{}:Oops", std::file!(), std::line!());
                }
            },
            HttpData::UpdateMedia(anime_ptr, entry) => {
                eprintln!("Update media request received... {}", entry.id());
                if let Some(anime) = app
                    .database
                    .anime_map()
                    .iter_mut()
                    .find(|v| v.as_ptr_id() == anime_ptr) {
                    if entry.updated_at() > anime.last_watched() {
                        eprintln!("Updated \"{}\"...", anime.title());
                        anime.set_last_watched(entry.updated_at());
                    }
                } else {
                    eprintln!("Failed to update {}", entry.id());
                }
            }
            HttpData::Debug(v) => {
                dbg!(v);
            }
        }
    }
}
