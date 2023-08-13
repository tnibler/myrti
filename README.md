# Personal photo and video library

## Rationale 

Browsing a large(-ish) family media library in a file browser is a bit of a pain,
especially on a remote NAS.
There's no thumbnails, videos take long to start playing and may not play at all
depending on your connection. Then there's probably some HEVC, HEIF et. al sprinkled around
which many devices can't display at all.

Organizing is a bit clumsy and limited, since a file can only be in one directory at one,
and forget about searching.

Existing media library and management systems all have dealbreakers for me personally.
Here is an excerpt:

 - Proprietary clients and/or backends
 - Requiring "uploading" files to them, and in general taking ownership of your media.
 The library I want to browse is shared with other people, and I don't want to force
 them to change their ways (everyone already knows how to load photos from an SD
 card onto the NAS).
 I also want to retain the existing folder structure, and will not give unknown software
 write access to our family NAS.
 - Attempting to reimplement Syncthing (and inevitably being worse)
 while also competing with Google Photos on all other fronts simultaneously
 - Requiring powerful hardware for baked-in machine learning functionality
 that either won't run at all or make my poor laptop server melt down.
 Compute is not cheap and neither is power.
 - Deployment involves a dance of containers, external services, networking and configuration
 to even try it out and see if the basic features are what you want.

## Goals 
 
 - Easy and non-annoying way to browse/organize a personal photo and video
 library in remote storage (like a NAS)
 - Can coexist with existing organization systems (like folder structure),
 does not take ownership or require ingesting files. Point it at your NAS and go!
 The index exists next to the original media (which requires more storage,
 but hard drives are cheap)
 - Makes watching videos remotely not consist of buffering and incompatible codecs
 - Practical to self-host on lower end hardware like a retired laptop.
 - Building and running a single binary is all that's required for core functionality
 and will always be supported
 - Extensible with all sorts of ML features you can think of, without making
 them mandatory. These should be services that can be run on other (more powerful)
 machines, such as your desktop with a GPU or even a cloud instance.

## Non-goals

 - End to end encryption (fundamentally incompatible with adaptive streaming),
 server and admin are trusted.
 - Backup and sync functionality. Syncthing exists, or rsync or Nextcloud, pick what you like.
 Also, by allowing this to run with read-only access to originals, I hope it lowers the barrier
 for users to try it out and doesn't mean every feature has to be aerospace grade right away
 as we don't risk borking valuable data.

## how to build

```
cargo install sqlx
export DATABASE_URL=sqlite://devel.db
cargo sqlx database create
cargo sqlx migrate run
cargo build
```

## run 

create a file `config.toml` in the repo checkout dir like this:

```
[[AssetDirs]]
name = "one dir"
path = "/path/to/media
[[AssetDirs]]
name = "two dir"
path = "/path/to/other/media"

[[DataDirs]]
path = "/where/can/this/dump/its/stuff"
```

```
export RUST_LOG="info,sqlx=info,hyper=info,tower_http=info"
cargo run
```

get assets
```
curl http://localhost:3000/api/asset
```

index a directory:
```
curl http://localhost:3000/api/indexAssetRoot?id=1 -X POST
```

look at a thumbnail
```
http://localhost:3000/api/asset/thumbnail/{id}/small/webp
http://localhost:3000/api/asset/thumbnail/{id}/large/jpb
```

stream a video
```
mpv http://localhost:3000/api/dash/{id}/stream.mpd
```
