use std::collections::{HashMap, HashSet};

use camino::Utf8PathBuf as PathBuf;
use chrono::{Months, Utc};
use claims::{assert_ok, assert_err};
use proptest::prelude::*;

use super::proptest_arb::{arb_new_album, arb_new_asset };
use super::util::prop_insert_create_test_assets;

use crate::model::{
    repository::{
        self,
        album::{CreateAlbum, CreateTimelineGroup},
    },
    CreateAssetBase, CreateAssetSpe, CreateAssetImage, CreateAssetVideo,
    Album, AlbumType, Asset, AssetBase, AssetId, AssetRootDir, AssetRootDirId, TimelineGroupAlbum, CreateAsset, AssetSpe, Image, TimestampInfo, Size, AssetType,
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
            ) -> (Vec<Asset>, Vec<(AlbumType, Vec<prop::sample::Index>)>) {
                (assets, albums_asset_idxs)
            }
    }
    proptest!(|((assets, albums_asset_idxs) in arb_assets_and_albums(root_dir_id),
    append_chunk_size in 1usize..5)| {
        rt.block_on(async {
            sqlx::query!(r#"DELETE FROM AlbumEntry; DELETE FROM Asset; DELETE FROM Album; "#).execute(&pool).await.unwrap();
            let assets_with_ids: Vec<Asset> = prop_insert_create_test_assets(&pool, &assets).await?;
            let mut albums_with_ids: Vec<AlbumType> = Vec::default();
            for (album, _) in &albums_asset_idxs {
                let (album_base, timeline_group) = match album {
                    AlbumType::Album(album) => (album, None),
                    AlbumType::TimelineGroup(tg) => (&tg.album, Some(&tg.group)),
                };
                let create_album = CreateAlbum {
                    name: album_base.name.clone(),
                    description: album_base.description.clone(),
                    timeline_group: timeline_group.map(|tg|CreateTimelineGroup { display_date: tg.display_date })
                };
                // TODO initial creation with assets to insert right away not tested here
                let album_insert_result = repository::album::create_album(&pool, create_album, &[]).await;
                prop_assert!(album_insert_result.is_ok());
                let album_id = album_insert_result.unwrap();
                let album_with_id = match album {
                    AlbumType::Album(album) => AlbumType::Album(Album { id: album_id, ..album.clone() }),
                    AlbumType::TimelineGroup(tga) => AlbumType::TimelineGroup(TimelineGroupAlbum { 
                        group: tga.group.clone(),
                        album: Album {
                            id: album_id,
                            ..tga.album.clone()} 
                    })
                };
                albums_with_ids.push(album_with_id);
            }
            let mut albums_assets_with_ids: Vec<(AlbumType, Vec<Asset>)> = Vec::default();
            let mut albums_by_asset: HashMap<AssetId, Vec<AlbumType>> = HashMap::default();
            for (album, (_album_no_id, asset_idxs)) in albums_with_ids.iter().zip(albums_asset_idxs.iter()) {
                // removing duplicates
                let assets_to_append: Vec<Asset> = asset_idxs.iter().map(|idx| idx.get(&assets_with_ids).clone()).into_iter().collect::<HashSet<_>>().into_iter().collect();
                let mut assets_actually_appended: Vec<Asset> = Vec::default();
                let append_chunks: Vec<&[Asset]> = assets_to_append.chunks(append_chunk_size).collect();
                for chunk in append_chunks {
                    let mut tx = pool.begin().await.unwrap();
                    let any_asset_already_in_group = chunk.iter()
                        // get albums that this asset is in
                        .any(|asset| albums_by_asset.get(&asset.base.id).unwrap_or(&Vec::default())
                            // are any of them a group?
                            .into_iter().any(|album| matches!(album, AlbumType::TimelineGroup(_)))
                        );
                    let album_is_group = matches!(album, AlbumType::TimelineGroup(_));
                    let chunk_ids: Vec<AssetId> = chunk.into_iter().map(|asset| asset.base.id).collect();
                    let append_result = repository::album::append_assets_to_album(tx.as_mut(), album.album_base().id, &chunk_ids).await;
                    // if any asset was already in a group album and the album we're inserting into
                    // is also a group, adding to the album should fail
                    prop_assert!(tx.commit().await.is_ok());
                    if !(album_is_group && any_asset_already_in_group) {
                        prop_assert!(append_result.is_ok());
                        assets_actually_appended.extend_from_slice(chunk);
                        chunk_ids.iter().for_each(|asset_id| match albums_by_asset.get_mut(asset_id) {
                            None => {
                                albums_by_asset.insert(*asset_id, vec![album.clone()]);
                            },
                            Some(ref mut albums) => albums.push(album.clone())
                        });

                    } else {
                        prop_assert!(append_result.is_err());
                    }
                }
                albums_assets_with_ids.push((album.clone(), assets_actually_appended));
            }
            for (album, expected_assets) in albums_assets_with_ids {
                let retrieve_result = repository::album::get_assets_in_album(album.album_base().id, &pool).await;
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

#[tokio::test]
#[allow(unused_must_use)]
async fn adding_asset_to_multiple_group_albums_fails() {
    let pool = create_db().await;
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let root_dir_id =
    assert_ok!(repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir).await);
    let asset = CreateAsset {
        spe: CreateAssetSpe::Image(CreateAssetImage {
            image_format_name: "jpeg".into(),
        }),
        base:CreateAssetBase {
            root_dir_id,
            file_type: "jpeg".to_owned(),
            file_path: PathBuf::from("image.jpg"),
            taken_date: utc_now_millis_zero()
                .checked_sub_months(Months::new(2))
                .unwrap(),
            timestamp_info: TimestampInfo::UtcCertain,
            size: Size {
                width: 1024,
                height: 1024,
            },
            rotation_correction: None,
            hash: None,
            gps_coordinates: None,
        }
    };
    let asset2 = CreateAsset {
        spe: CreateAssetSpe::Image(CreateAssetImage {
            image_format_name: "jpeg".into(),
        }),
        base: CreateAssetBase {
            root_dir_id,
            file_type: "jpeg".to_owned(),
            file_path: PathBuf::from("image2.jpg"),
            taken_date: utc_now_millis_zero()
                .checked_sub_months(Months::new(2))
                .unwrap(),
            timestamp_info: TimestampInfo::UtcCertain,
            size: Size {
                width: 1024,
                height: 1024,
            },
            rotation_correction: None,
            hash: None,
            gps_coordinates: None,
        }
    };
    let asset_id = assert_ok!(repository::asset::create_asset(&pool, asset.clone()).await);
    let asset2_id = assert_ok!(repository::asset::create_asset(&pool, asset2.clone()).await);
    let album1 = CreateAlbum {
        name: Some("asdf".into()),
        description: None,
        timeline_group: None
    };
    let album2 = CreateAlbum {
        name: Some("asdf2".into()),
        description: None,
        timeline_group: None
    };
    let album_group = CreateAlbum {
        name: Some("group1".into()),
        description: None,
        timeline_group: Some(CreateTimelineGroup { display_date: Utc::now() })
    };
    let album_group2 = CreateAlbum {
        name: Some("group2".into()),
        description: None,
        timeline_group: Some(CreateTimelineGroup { display_date: Utc::now() })
    };
    let album1_id = assert_ok!(repository::album::create_album(&pool, album1, &[]).await);
    let album2_id = assert_ok!(repository::album::create_album(&pool, album2, &[]).await);
    let album_group_id = assert_ok!(repository::album::create_album(&pool, album_group, &[]).await);
    let album_group2_id = assert_ok!(repository::album::create_album(&pool, album_group2, &[]).await);
    assert_ok!(repository::album::append_assets_to_album(pool.acquire().await.unwrap().as_mut(), album1_id, &[asset_id]).await);
    assert_ok!(repository::album::append_assets_to_album(pool.acquire().await.unwrap().as_mut(), album2_id, &[asset_id]).await);
    assert_ok!(repository::album::append_assets_to_album(pool.acquire().await.unwrap().as_mut(), album_group_id, &[asset_id]).await);
    assert_err!(repository::album::append_assets_to_album(pool.acquire().await.unwrap().as_mut(), album_group2_id, &[asset_id]).await);
    let ret_album1: Vec<AssetId> = assert_ok!(repository::album::get_assets_in_album(album1_id, &pool).await).into_iter().map(|asset| asset.base.id).collect();
    let ret_album2: Vec<AssetId> = assert_ok!(repository::album::get_assets_in_album(album2_id, &pool).await).into_iter().map(|asset| asset.base.id).collect();
    let ret_album_group: Vec<AssetId> = assert_ok!(repository::album::get_assets_in_album(album_group_id, &pool).await).into_iter().map(|asset| asset.base.id).collect();
    let ret_album_group2: Vec<AssetId> = assert_ok!(repository::album::get_assets_in_album(album_group2_id, &pool).await).into_iter().map(|asset| asset.base.id).collect();
    assert_eq!(ret_album1, vec![asset_id]);
    assert_eq!(ret_album2, vec![asset_id]);
    assert_eq!(ret_album_group, vec![asset_id]);
    assert_eq!(ret_album_group2, vec![]);
    assert_ok!(repository::album::append_assets_to_album(pool.acquire().await.unwrap().as_mut(), album_group_id, &[asset2_id]).await);
}
