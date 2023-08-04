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
