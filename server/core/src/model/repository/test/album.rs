use std::collections::{HashMap, HashSet};

use camino::Utf8PathBuf as PathBuf;
use claims::assert_ok;
use proptest::prelude::*;

use super::proptest_arb::{arb_new_album, arb_new_asset};
use super::util::prop_insert_create_test_assets;

use crate::model::{
    repository::{self, album::CreateAlbum},
    Album, Asset, AssetId, AssetRootDir, AssetRootDirId,
};

use super::*;

#[test]
fn prop_create_retrieve_albums() {
    let rt = tokio::runtime::Runtime::new().unwrap();
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
            assets in prop::collection::vec(arb_new_asset(root_dir_id), 1..5),
            albums in prop::collection::vec(arb_new_album(), 1..2)
        )
            (
                albums_asset_idxs in albums.into_iter().map(|album| (Just(album), prop::collection::vec(any::<prop::sample::Index>(), 0..assets.len()))).collect::<Vec<_>>(),
                assets in Just(assets),
            ) -> (Vec<Asset>, Vec<(Album, Vec<prop::sample::Index>)>) {
                (assets, albums_asset_idxs)
            }
    }
    proptest!(|((assets, albums_asset_idxs) in arb_assets_and_albums(root_dir_id),
    append_chunk_size in 1usize..5)| {
        rt.block_on(async {
            sqlx::query!(r#"DELETE FROM AlbumEntry; DELETE FROM Asset; DELETE FROM Album; "#).execute(&pool).await.unwrap();
            let assets_with_ids: Vec<Asset> = prop_insert_create_test_assets(&pool, &assets).await?;
            let mut albums_with_ids: Vec<Album> = Vec::default();
            for (album, _) in &albums_asset_idxs {
                let create_album = CreateAlbum {
                    name: album.name.clone(),
                    description: album.description.clone(),
                };
                // TODO initial creation with assets to insert right away not tested here
                let album_insert_result = repository::album::create_album(&pool, create_album, &[]).await;
                prop_assert!(album_insert_result.is_ok());
                let album_id = album_insert_result.unwrap();
                let album_with_id = Album { id: album_id, ..album.clone() };
                albums_with_ids.push(album_with_id);
            }
            let mut albums_assets_with_ids: Vec<(Album, Vec<Asset>)> = Vec::default();
            let mut albums_by_asset: HashMap<AssetId, Vec<Album>> = HashMap::default();
            for (album, (_album_no_id, asset_idxs)) in albums_with_ids.iter().zip(albums_asset_idxs.iter()) {
                // removing duplicates
                let assets_to_append: Vec<Asset> = asset_idxs.iter().map(|idx| idx.get(&assets_with_ids).clone()).into_iter().collect::<HashSet<_>>().into_iter().collect();
                let mut assets_actually_appended: Vec<Asset> = Vec::default();
                let append_chunks: Vec<&[Asset]> = assets_to_append.chunks(append_chunk_size).collect();
                for chunk in append_chunks {
                    let mut tx = pool.begin().await.unwrap();
                    let chunk_ids: Vec<AssetId> = chunk.into_iter().map(|asset| asset.base.id).collect();
                    let append_result = repository::album::append_assets_to_album(tx.as_mut(), album.id, &chunk_ids).await;
                    // if any asset was already in a group album and the album we're inserting into
                    // is also a group, adding to the album should fail
                    prop_assert!(tx.commit().await.is_ok());
                    prop_assert!(append_result.is_ok());
                    assets_actually_appended.extend_from_slice(chunk);
                    chunk_ids.iter().for_each(|asset_id| match albums_by_asset.get_mut(asset_id) {
                        None => {
                            albums_by_asset.insert(*asset_id, vec![album.clone()]);
                        },
                        Some(ref mut albums) => albums.push(album.clone())
                    });
                }
                albums_assets_with_ids.push((album.clone(), assets_actually_appended));
            }
            for (album, expected_assets) in albums_assets_with_ids {
                let retrieve_result = repository::album::get_assets_in_album(album.id, &pool).await;
                prop_assert!(retrieve_result.is_ok());
                let actual_assets_in_album: Vec<Asset> = retrieve_result.unwrap();
                let expected_indices: Vec<usize> = (0..expected_assets.len()).collect();
                prop_assert_eq!(expected_assets, actual_assets_in_album);
                let actual_indices: Vec<usize> = sqlx::query!(r#"
                SELECT ae.idx as idx FROM Album, AlbumEntry ae WHERE ae.album_id=Album.id ORDER BY ae.idx;
                "#).fetch_all(&pool)
                    .await
                    .map(|rows| rows.into_iter().map(|row| row.idx as usize).collect::<Vec<_>>())
                    .unwrap();
                prop_assert_eq!(expected_indices, actual_indices);
            }
            Ok(())
        })?;
    })
}
