use claims::assert_ok;
use pretty_assertions::assert_eq;

use proptest::prelude::*;
use proptest_arb::{arb_new_album_timeline_group, arb_new_asset, prop_insert_albums_add_assets};

#[test]
fn prop_test_timeline() {
    // generate assets
    // pick random distinct subsets
    // create group albums from subsets, assigning random display_dates
    // query timeline in different chunk sizes
    //
    // invariants in incresing order of complexity:
    //  - no duplicate assets between chunks
    //  - date ordering
    //  - all assets contained in chunks
    //  - chunks are correct
    let pool = rt.block_on(async { create_db().await });
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let root_dir_id = assert_ok!(rt.block_on(async {
        repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir).await
    }));
    prop_compose! {
        fn arb_assets_and_albums(root_dir_id: AssetRootDirId)
        (
            assets in prop::collection::vec(arb_new_asset(root_dir_id), 1..500),
            albums in prop::collection::vec(arb_new_album_timeline_group(), 0..50)
        )
            (
                albums_asset_idxs in albums.into_iter().map(|album| (Just(album), prop::collection::vec(any::<prop::sample::Index>(), 0..assets.len()))).collect::<Vec<_>>(),
                assets in Just(assets),
            ) -> (Vec<Asset>, Vec<(AlbumType, Vec<prop::sample::Index>)>) {
                (assets, albums_asset_idxs)
            }
    }
    proptest!(|((assets, album_asset_idxs) in arb_assets_and_albums(root_dir_id))| {
        sqlx::query!(r#"DELETE FROM AlbumEntry; DELETE FROM Asset; DELETE FROM Album; "#).execute(&pool).await.unwrap();
        let assets_with_ids: Vec<Asset> = prop_insert_create_test_assets(pool, &assets).await?;
        let albums_with_ids: Vec<TimelineGroupAlbum> = prop_insert_albums_add_assets(pool, &assets_with_ids, &albums_asset_idxs).await?
        .into_iter().map(|album| match album {
            AlbumType::TimelineGroup(tg) => th,
            _ => panic!("wrong album type")
        }).collect();
    });
}
