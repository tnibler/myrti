# Server

Built with [axum](https://docs.rs/axum/latest/axum/), SQLite and [diesel](https://diesel.rs/).
Crates are split mostly for compile times right now:
the `myrti-data` crate contains everything that touches diesel, `server` has the binary and lots of proc macros, `myrti-core` is everything else.

The database schema is defined in `myrti-data/migrations/*/up.sql` and repeated in `myrti-data/src/repository/schema.rs` for diesel.
The core tables are:

 - `Asset`: one logical image or video. Assets are conceptually unique, different versions of the same photo are still a single Asset
 - `AssetFile`, `ImageFile`, `VideoFile`: files have a 1:N relationship with Assets
 - `AssetSeries`: images that are very similar or form a sequence form a series (displayed as a stack). One or more assets within a series can be marked as good (selection) and the stack will be split so that every marked image is shown in the timeline.
 - `TimelineGroup`: assets can be grouped and given a title, they will be displayed together in the timeline. 
 There might be more than one timeline one day, which is why this relationship goes through the `TimelineGroupItem` table.

`Album` and `AlbumItem` are self-explanatory, though that albums are broken right now and are low priority.

The timeline is precomputed, the code for that is in `myrti-data/src/repository/rebuild_timeline_*.sql`.

`TimelineItem`s are divided into segments that represent either one day or one `TimelineGroup`.
Sections are the subdivision used for loading only parts of the timeline on the client, 
they are independent of dates and months so a folder with 10k assets with the same timestamp doesn't break the paging mechanism.

`TimelineMonth` holds summary information by year and month for the scrubbing scrollbar, 
which needs to know approximately how much vertical space a month takes up.


## Tests

Besides small unit tests, there is a basic integration test setup too.
The directory with test assets is currently not shared for space and potential copyright reasons, but the directory structure is reproduced in `test-data/fake-tree`.
`ffprobe` and `exiftool` are replaced with wrapper scripts that run the real command on real files if `MYRTI_TEST_REAL_DATA` is set,
and the output is saved to be replayed on subsequent runs.

`cargo test` runs all tests.

## API Routes

Endpoints are defined in `server/src/routes` and use types from the `server` crate only, not `myrti-core`.
By running `just openapi`, the OpenAPI schema is regenerated as well as `zod` typescript definitions (using `orval`),
so request and response bodies are only defined in one place and everything is type safe.

# Web client

The frontend uses Svelte 5 and simple client side routing, no SvelteKit or similar.
There's not a lot there besides the fancy gallery widget and the timeline grid.

Nothing is set in stone here and I will gladly give away ownership of almost everything.
