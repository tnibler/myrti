use camino::Utf8PathBuf as PathBuf;
use claims::assert_ok;
use proptest::prelude::*;

use super::proptest_arb::{arb_new_album, arb_new_asset};

use crate::model::{repository, Album, Asset, AssetBase, AssetId, AssetRootDir, AssetRootDirId};

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
            assets in prop::collection::vec(arb_new_asset(root_dir_id), 0..1),
            albums in prop::collection::vec(arb_new_album(), 0..1)
        )
        (
            albums_asset_idxs in albums.into_iter().map(|album| (Just(album), prop::collection::vec(any::<prop::sample::Index>(), 0..30))).collect::<Vec<_>>(),
            assets in Just(assets),
        ) -> (Vec<Asset>, Vec<(Album, Vec<prop::sample::Index>)>) {
            (assets, albums_asset_idxs)
        }
    }
    proptest!(|((assets, albums_asset_idxs) in arb_assets_and_albums(root_dir_id))| {
        rt.block_on(async {
            let mut assets_with_ids: Vec<Asset> = Vec::default();
            for asset in &assets {
                let asset_insert_result = repository::asset::insert_asset(&pool, &asset).await;
                prop_assert!(asset_insert_result.is_ok());
                let asset_id = asset_insert_result.unwrap();
                let asset_with_id = Asset {
                    base: AssetBase {
                        id: asset_id,
                        ..asset.base.clone()
                    },
                    ..asset.clone()
                };
                assets_with_ids.push(asset_with_id.into());
            }
            let mut albums_with_ids: Vec<Album> = Vec::default();
            for (album, _) in &albums_asset_idxs {
                let album_insert_result = repository::album::insert_album(&pool, album).await;
                prop_assert!(album_insert_result.is_ok());
                let album_id = album_insert_result.unwrap();
                let album_with_id = Album {
                    id: album_id,
                    ..album.clone()
                };
                albums_with_ids.push(album_with_id);
            }
            let mut albums_assets_with_ids: Vec<(Album, Vec<Asset>)> = Vec::default();
            for (album, (_album_no_id, asset_idxs)) in albums_with_ids.iter().zip(albums_asset_idxs.iter()) {
                let assets_to_append: Vec<Asset> = asset_idxs.iter().map(|idx| idx.get(&assets_with_ids).clone()).collect();
                let asset_ids_to_append: Vec<AssetId> = assets_to_append.iter().map(|asset| asset.base.id).collect();
                let append_chunks: Vec<&[AssetId]> = asset_ids_to_append.chunks(3).collect();
                for chunk in append_chunks {
                    let append_result = repository::album::append_assets_to_album(&pool, album.id, chunk.iter().cloned()).await;
                    prop_assert!(append_result.is_ok());
                }
                albums_assets_with_ids.push((album.clone(), assets_to_append));
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
