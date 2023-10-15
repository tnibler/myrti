use std::collections::HashSet;

use claims::{assert_ok};
use camino::Utf8PathBuf as PathBuf;
use proptest::prelude::*;

use super::proptest_arb::{arb_new_asset, arb_new_album};

use crate::{
    catalog::storage_key,
    core::storage,
    model::{
        repository, Album, Asset, AssetBase, AssetId, AssetRootDir, AssetRootDirId, AssetSpe, AssetType,
        CreateAsset, Image
    },
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
            for album in &albums_with_ids {
                let append_result = repository::album::append_assets_to_album(&pool, album.id, assets_with_ids.iter().map(|a| a.base.id)).await;
                prop_assert!(append_result.is_ok());
            }
            // for (album, (album_no_id, asset_idxs)) in albums_with_ids.iter().zip(albums_asset_idxs.iter()) {
            //     let asset_ids_to_append: Vec<AssetId> = asset_idxs.iter().map(|idx| idx.get(&assets_with_ids).base.id).collect();
            //     let append_result = repository::album::append_assets_to_album(&pool, album.id, asset_ids_to_append).await;
            //     // let append_chunks: Vec<&[AssetId]> = asset_ids_to_append.chunks(3).collect();
            //     prop_assert!(append_result.is_ok());
            //     // for chunk in append_chunks {
            //     //     let append_result = repository::album::append_assets_to_album(&pool, album.id, chunk.iter().cloned()).await;
            //     //     prop_assert!(append_result.is_ok());
            //     // }
            // }
            Ok(())
        })?;
    })
}
