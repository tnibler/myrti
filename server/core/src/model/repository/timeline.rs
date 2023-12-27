use eyre::{Context, Result};
use sqlx::{Decode, Row};

use crate::model::{
    repository::{
        album,
        db_entity::{DbAsset, DbAssetType, DbTimestampInfo},
    },
    AlbumId, AlbumType, Asset, AssetId, AssetRootDirId, TimelineGroupAlbum,
};

use super::{album::get_album, pool::DbPool};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TimelineElement {
    DayGrouped(Vec<Asset>),
    Group {
        group: TimelineGroupAlbum,
        assets: Vec<Asset>,
    },
}

impl TimelineElement {
    pub fn get_assets(&self) -> &[Asset] {
        match self {
            TimelineElement::DayGrouped(assets) => &assets,
            TimelineElement::Group { group: _, assets } => &assets,
        }
    }
}

macro_rules! dbasset_from_row {
    (&$row:ident) => {
        DbAsset {
            id: AssetId($row.id),
            ty: $row.ty,
            root_dir_id: AssetRootDirId($row.root_dir_id),
            file_type: $row.file_type,
            file_path: $row.file_path,
            is_hidden: $row.is_hidden,
            hash: $row.hash,
            added_at: $row.added_at,
            taken_date: $row.taken_date,
            timezone_offset: $row.timezone_offset,
            timezone_info: $row.timezone_info,
            width: $row.width,
            height: $row.height,
            rotation_correction: $row.rotation_correction,
            gps_latitude: $row.gps_latitude,
            gps_longitude: $row.gps_longitude,
            thumb_small_square_avif: $row.thumb_small_square_avif,
            thumb_small_square_webp: $row.thumb_small_square_webp,
            thumb_large_orig_avif: $row.thumb_large_orig_avif,
            thumb_large_orig_webp: $row.thumb_large_orig_webp,
            thumb_small_square_width: $row.thumb_small_square_width,
            thumb_small_square_height: $row.thumb_small_square_height,
            thumb_large_orig_width: $row.thumb_large_orig_width,
            thumb_large_orig_height: $row.thumb_large_orig_height,
            image_format_name: $row.image_format_name,
            video_codec_name: $row.video_codec_name,
            video_bitrate: $row.video_bitrate,
            audio_codec_name: $row.audio_codec_name,
            has_dash: $row.has_dash,
        }
    };
}

// TODO when the time comes: figure out how to handle dates of different timezones.
// Right now assets are grouped by Asset::taken_date_local().date_naive() and sorted by timestamp,
// which may or may not by what we want.
#[tracing::instrument(skip(pool), level = "debug")]
pub async fn get_timeline_chunk(
    pool: &DbPool,
    last_id: Option<AssetId>,
    max_count: i64,
) -> Result<Vec<TimelineElement>> {
    // timezone to calculate local dates in
    let timezone = &chrono::Local;
    let assets_albumid = sqlx::query!(
        r#"
WITH last_asset AS
(
    SELECT Asset.*, 
    CASE WHEN Album.id IS NOT NULL THEN Album.id ELSE 0 END AS album_id,
    CASE WHEN Album.id IS NOT NULL THEN Album.timeline_group_display_date ELSE Asset.taken_date END AS sort_group_date
    FROM Asset
    LEFT JOIN AlbumEntry ON AlbumEntry.asset_id = Asset.id
    LEFT JOIN Album ON AlbumEntry.album_id = Album.id
    WHERE Asset.id = $1
    AND (Album.is_timeline_group = 1 OR Album.id IS NULL)
)
SELECT
Asset.id,
Asset.ty as "ty: DbAssetType",
Asset.root_dir_id,
Asset.file_path,
Asset.file_type,
Asset.hash,
Asset.is_hidden,
Asset.added_at,
Asset.taken_date,
Asset.timezone_offset,
Asset.timezone_info as "timezone_info: DbTimestampInfo",
Asset.width,
Asset.height,
Asset.rotation_correction as "rotation_correction: i32",
Asset.gps_latitude as "gps_latitude: i64",
Asset.gps_longitude as "gps_longitude: i64",
Asset.thumb_small_square_avif ,
Asset.thumb_small_square_webp, 
Asset.thumb_large_orig_avif ,
Asset.thumb_large_orig_webp ,
Asset.thumb_small_square_width,
Asset.thumb_small_square_height,
Asset.thumb_large_orig_width,
Asset.thumb_large_orig_height,
Asset.image_format_name,
Asset.video_codec_name,
Asset.video_bitrate,
Asset.audio_codec_name,
Asset.has_dash,
CASE WHEN Album.id IS NOT NULL THEN Album.id ELSE 0 END AS album_id,
CASE WHEN Album.id IS NOT NULL THEN Album.timeline_group_display_date ELSE Asset.taken_date END AS "sort_group_date: i64"
FROM Asset
LEFT JOIN AlbumEntry ON AlbumEntry.asset_id = Asset.id
LEFT JOIN Album ON AlbumEntry.album_id = Album.id
WHERE
(Album.is_timeline_group = 1 OR Album.id IS NULL)
AND
(
    ($1 IS NULL)
    OR 
    ("sort_group_date: i64", album_id, Asset.taken_date, Asset.id) < (SELECT sort_group_date, album_id, taken_date, id FROM last_asset)
    OR
    (
    album_id IS NULL AND (SELECT album_id FROM last_asset) IS NULL
    AND
    ("sort_group_date: i64", Asset.taken_date, Asset.id) < (SELECT sort_group_date, taken_date, id FROM last_asset)
    )
)
ORDER BY "sort_group_date: i64" DESC, album_id DESC, Asset.taken_date DESC, Asset.id DESC
LIMIT $2;
    "#,
        last_id,
        max_count
    )
        .fetch_all(pool)
        .await
        .wrap_err("timeline query failed")?
        .into_iter()
        .map(|row| {
            // println!("{:?} {:?} {:?} {:?}", row.id, row.sort_group_date, row.album_id, row.taken_date);
            let asset = dbasset_from_row!(&row);
            let album_id = match row.album_id {
                0 => None,
                id => Some(AlbumId(id as i64))
            };
            (asset, album_id)
        });
    let mut timeline_els: Vec<TimelineElement> = Vec::default();
    for (db_asset, album_id) in assets_albumid {
        let asset: Asset = db_asset.try_into()?;
        let mut last_el = timeline_els.last_mut();
        match &mut last_el {
            None => {
                // create new TimelineElement
                let new_el = if let Some(album_id) = album_id {
                    let group = match get_album(pool, album_id).await? {
                        AlbumType::TimelineGroup(group) => group,
                        _ => unreachable!("TODO this is getting removed anyhow"),
                    };
                    TimelineElement::Group {
                        group,
                        assets: vec![asset],
                    }
                } else {
                    TimelineElement::DayGrouped(vec![asset])
                };
                timeline_els.push(new_el);
            }
            Some(ref mut last_el) => match (last_el, album_id) {
                // Matching cases: add this asset to last TimelineElement
                (TimelineElement::DayGrouped(ref mut assets), None)
                    if assets
                        .last()
                        .map(|a| {
                            a.base.taken_date.with_timezone(timezone).date_naive()
                                == asset.base.taken_date.with_timezone(timezone).date_naive()
                        })
                        // .unwrap_or(true)
                        .expect("There should never be an empty DayGrouped") =>
                {
                    assets.push(asset);
                }
                (TimelineElement::Group { group, assets }, Some(album_id))
                    if group.album.id == album_id =>
                {
                    assets.push(asset);
                }
                // Need to create new TimelineElement for these cases
                (_, Some(album_id)) => {
                    let group = match get_album(pool, album_id).await? {
                        AlbumType::TimelineGroup(group) => group,
                        _ => unreachable!("TODO this is getting removed anyhow"),
                    };
                    timeline_els.push(TimelineElement::Group {
                        group,
                        assets: vec![asset],
                    });
                }
                // last DayGroup element does not match this date
                (_, None) => timeline_els.push(TimelineElement::DayGrouped(vec![asset])),
            },
        };
    }
    Ok(timeline_els)
}
