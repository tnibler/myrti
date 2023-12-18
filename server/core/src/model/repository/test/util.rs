use proptest::prelude::*;

use crate::model::{
    repository::{
        self,
        album::{CreateAlbum, CreateTimelineGroup},
        pool::DbPool,
    },
    Album, AlbumId, AlbumType, Asset, AssetBase, AssetId, AssetSpe, TimelineGroupAlbum,
};

/// Inserts asset and returns them in the same order, with asset_id set
/// For VideoAssets, it uses an empty string as ffprobe_output
pub async fn prop_insert_create_test_assets(
    pool: &DbPool,
    assets: &[Asset],
) -> Result<Vec<Asset>, TestCaseError> {
    let mut assets_with_ids: Vec<Asset> = Vec::default();
    for asset in assets {
        let ffprobe_output: Option<&[u8]> = match &asset.sp {
            AssetSpe::Video(_video) => Some(&[]),
            _ => None,
        };
        #[allow(deprecated)]
        let asset_insert_result =
            repository::asset::insert_asset(&pool, &asset, ffprobe_output).await;
        prop_assert!(
            asset_insert_result.is_ok(),
            "Inserting Asset returned error: {}",
            asset_insert_result.unwrap_err()
        );
        let asset_id = asset_insert_result.unwrap();
        let asset_with_id = Asset {
            base: AssetBase {
                id: asset_id,
                ..asset.base.clone()
            },
            ..asset.clone()
        };
        prop_assert_ne!(asset_with_id.base.id, AssetId(0));
        assets_with_ids.push(asset_with_id.into());
    }
    Ok(assets_with_ids)
}

/// Inserts empty albums, then adds assets to them
/// returns albums in same order with album_id set
pub async fn prop_insert_albums_add_assets(
    pool: &DbPool,
    assets: &[Asset],
    albums_asset_idxs: &[(AlbumType, Vec<prop::sample::Index>)],
) -> Result<Vec<AlbumType>, TestCaseError> {
    let mut albums_with_ids: Vec<AlbumType> = Vec::default();
    for (album, asset_idxs) in albums_asset_idxs {
        let assets: Vec<AssetId> = asset_idxs
            .iter()
            .map(|idx| idx.get(assets).base.id)
            .collect();
        let album_with_id =
            prop_insert_album_add_assets(pool, album, assets.iter().copied()).await?;
        albums_with_ids.push(album_with_id);
    }
    Ok(albums_with_ids)
}

pub async fn prop_insert_album_add_assets(
    pool: &DbPool,
    album: &AlbumType,
    asset_ids: impl Iterator<Item = AssetId>,
) -> Result<AlbumType, TestCaseError> {
    let (album_base, timeline_group) = match album {
        AlbumType::Album(album) => (album, None),
        AlbumType::TimelineGroup(tg) => (&tg.album, Some(&tg.group)),
    };
    let create_album = CreateAlbum {
        name: album_base.name.clone(),
        description: album_base.description.clone(),
        timeline_group: timeline_group.map(|tg| CreateTimelineGroup {
            display_date: tg.display_date,
        }),
    };
    // initial creation with assets to insert right away not tested here
    let album_insert_result = repository::album::create_album(&pool, create_album, &[]).await;
    prop_assert!(
        album_insert_result.is_ok(),
        "Inserting Album returned error: {}",
        album_insert_result.unwrap_err()
    );
    let album_id = album_insert_result.unwrap();
    let album_with_id = match album {
        AlbumType::Album(album) => AlbumType::Album(Album {
            id: album_id,
            ..album.clone()
        }),
        AlbumType::TimelineGroup(tga) => AlbumType::TimelineGroup(TimelineGroupAlbum {
            group: tga.group.clone(),
            album: Album {
                id: album_id,
                ..tga.album.clone()
            },
        }),
    };
    let mut tx = pool.begin().await.unwrap();
    prop_assert_ne!(album_with_id.album_base().id, AlbumId(0));
    let append_result = repository::album::append_assets_to_album(
        tx.as_mut(),
        album_with_id.album_base().id,
        &asset_ids.collect::<Vec<_>>(),
    )
    .await;
    prop_assert!(
        append_result.is_ok(),
        "Appending to Album returned error: {:?}",
        append_result.unwrap_err()
    );
    let commit_result = tx.commit().await;
    prop_assert!(
        commit_result.is_ok(),
        "Committing transaction returned error: {:?}",
        commit_result.unwrap_err()
    );
    Ok(album_with_id)
}
