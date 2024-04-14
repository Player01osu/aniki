
# Aniki

##### Warning: This is unfinished software! Don't expect everything to be there! Feedback and bug reports are appreciated.

Watch and keep track of anime.

(Written アニ記)

## Features

<details> <summary>Keep track of last anime watched and last episode watched</summary>
<video src="https://github.com/Player01osu/aniki/assets/85573610/2674337a-007c-4561-9fe7-0bdf0beb812b">
track anime
</video>
</details>

<details> <summary>Alias anime titles</summary>
<video src="https://github.com/Player01osu/aniki/assets/85573610/d99b0835-3549-43dd-b97c-adf781290025">
alias titles
</video>
</details>

<details> <summary>Choose thumbnails</summary>
<video src="https://github.com/Player01osu/aniki/assets/85573610/e5585a5e-0b90-4ef9-a607-859391f83e8c">
thumbnail
</video>
</details>

<details> <summary>Change anime (in case of incorrect detection)</summary>
<video src="https://github.com/Player01osu/aniki/assets/85573610/6af020b4-5e06-44cd-bffe-5c7679f47f05">
change anime
</video>
</details>

<details> <summary>Media detection</summary> </details>

<details> <summary>Sync with anime trackers</summary>
    <ul><li>
    <item><a href="https://anilist.co">Anilist</a>
    </li></ul>
</details>

### Coming Soon

- Configure directly in app

- Get synopsis from anilist/mal

- Sync with mal

- Custom styling

## Quickstart

Requirements:
- SDL2 >= 2.0.14
- SDL_Image >= 3.0
- SDL_ttf >= 3.0
- Cargo/rust >= 2021

Copy [aniki.conf](/aniki.conf) into `~/.config/aniki/aniki.conf` and change
`video_paths` to your anime folder path.

```console
cargo b --release
```
```console
./target/release/aniki
```

## Motivation

A small program that can organize and track my Anime in a clean and simple UI.

## Todo

- [ ] Cleanup
- [ ] Statically Link

## Related Projects

- [Taiga](https://taiga.moe/) - Anime sync and media detecting desktop application for Windows
- [sani](https://github.com/Player01osu/sani-desu) - Local anime tracker using dmenu frontend
