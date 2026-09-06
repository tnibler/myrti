<p align="center">
  <h3 align="center">
    myrti turns your photo dump into a friendly gallery
  </h3>

  <video align="center" width="800" src="https://github.com/user-attachments/assets/2a232517-3e86-48cc-8caf-4c6682b8a916"></video>
  <p align="center">Example timeline with 50k photos and videos</p>
</p>

An organized library is great in theory, but it takes a lot of work and discipline and is realistically never going to happen.
Unfortunately, that means the family NAS is a black hole and people never look through their photos and that's a shame.

This project aims to make existing libraries a joy to browse, requiring as little effort as possible for both users and administrators.

Myrti is read only and will never touch original files.
It's a replacement for SMB + File explorer, not Google Photos.
Nobody has to change any habits (copying from an SD card to NAS/cloud works as before) and using it is completely optional.

## Building and Running

With nix: `nix run .#server -- --config path/to/config`. This is currently the only build method that's always up to date.

Runtime dependencies: `ffmpeg`, `exiftool`, `gpac`, `libvips`. There are important fixes in the latest versions of `gpac` and `libvips`, distribution repos might be outdated.

Build dependencies: pnpm, Rust, C compiler, `pkg-config`, headers for `libvips`, `glib-2.0` and `gobject-2.0`.

To build static files for the web frontend (`web/dist`):

```shell
cd myrti/web
pnpm install
pnpm build
```

Then build the server:

```shell
cd myrti/server
export MYRTI_LOG=debug # optional
cargo run --release --bin server -- --config path/to/config --serve-static ../web/dist
```

Then navigate to the configured URL (`http://localhost:3000` by default) in a browser.

#### Configuration file

It's suggested to test on a few files only at first and see what happens.

```toml
# Interface to listen on
address = "0.0.0.0"
port = 3000

[DataDir]
# All application data is stored here including any transcoded videos and images,
# so ensure there's free space.
path = "/path/to/data"

[[AssetDirs]]
path = "/mnt/my-media-nas/"

# List of glob patterns to ignore when indexing
# exclude = ["boring/videos"]

# Excluded paths that match an include pattern are not ignored
# include = ["boring/videos/exceptions/*"]

[BinPaths]
# optionally override location of external programs
# ffmpeg = "..."
# ffprobe = "..."
# exiftool = "..."
# gpac = "..."
```

#### Frontend Dev Server

Vite can proxy requests to the server, if you changed the default port it needs to be set accordingly in `web/vite.config.ts`.

After starting the dev server with `pnpm run dev`, the application is served under `http://localhost:5173`.

## How is this different from Immich/Photoprism/...?

Scope and taste mostly. Other projects work great and are much more mature, but they're not quite what I want.

Immich for instance is a huge piece of software and does everything you could ever want, while myrti serves exactly one purpose:
a cutesy web gallery that's easy to casually look through, and quickly display the image or video you click on, always and without exception.

Making this experience as smooth and accessible as possible for everyone is the focus,
with an assumed ratio of "looking" to "doing" of 99:1 at least.

### Typical problems myrti aims to solve:

 - some phone photos and videos (HEIC, HEVC) don't work, and windows tells your gransparents they need to buy a codec
 - someone saved RAW files and they also don't work
 - there's 20,000 timelapse frames of clouds that make Explorer hang and nobody wants to see
 - when opening a video it takes too long to skip to the part where the thing happens
 - the video is in 4k and user is on mobile
 - user is on mobile and can't mount SMB shares
 - burst shots with 9 bad photos and 1 good one everywhere
 - the photo is upside down but the NAS is mounted as read only for some reason, so the rotate feature in the image viewer shows an error
 - a 500MB MOV video from 2008 doesn't play right

How myrti solves them:

 - save its own metadata separately without modifying originals: rotation correction, groupings and albums, hidden files
 - create thumbnails, transcode images and videos to supported formats when required 
 - stream videos over DASH for fast seeking and offer lower resolution versions. Storing copies is avoided when possible.
 - group/merge duplicates, near duplicates, RAW+JPEG versions, burst shots, timelapses etc.

Power users are explicitly not the target audience which means that there will be fewer features,
but also that the ones that do exist are designed for occasional use by people who don't care for computers,
not to be maximally efficient for expert users.

Examples of things that are out of scope (some for now, some always):

 - synchronization, backup, all of that
 - user accounts, permissions and access control
 - searching for outdoor portraits of dogs, shot on Nikon with an external flash
 - colour grading RAW photos
 - automation to set timezone metadata according to the directory name

Some of the guiding principles so far are:

 - less software is better, there's just one server binary (plus `ffmpeg` and co.). Pulling in e.g. Python to run ML models is out of the question.
 - do as much work as possible and prevent invalid state in the database, so working on the application level is low stakes and chill
 - build UX for people that learned to use a word processor in 2002 (whatever that entails)
 - optional features have to disappear when disabled, new buttons and menus shouldn't appear out of nowhere

## Project State

Everything is in its very early stages, but enough things work to start being useful. That means the main timeline, image and video conversion, DASH streaming and that's about it.
Code is locally messy everywhere, but as it becomes clearer how the whole thing should work it's ready to be cleaned up.

The web frontend is extremely bare-bones, and I don't have good enough taste to decide how UI should look and work.

_Contributing_: yes please!

### License

Files in this repository are available under the AGPLv3 license, except those where different terms are specified.
