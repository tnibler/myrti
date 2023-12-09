use chrono::{DateTime, Utc};
use eyre::{eyre, Context, Result};
use tracing::{debug, error};

use crate::model::{
    repository::{
        album, asset,
        db_entity::{DbAlbum, DbAsset},
    },
    util::datetime_to_db_repr,
    AlbumId, AlbumType, Asset, AssetId, TimelineGroupAlbum,
};

use super::{album::get_assets_in_album, pool::DbPool, DbError};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TimelineElement {
    DayGrouped(Vec<Asset>),
    Group {
        group: TimelineGroupAlbum,
        assets: Vec<Asset>,
    },
}

pub async fn get_timeline_chunk(
    pool: &DbPool,
    last_id: Option<AssetId>,
    max_count: i64,
) -> Result<Vec<TimelineElement>> {
    let mut result: Vec<TimelineElement> = Vec::default();
    let mut result_assets_size: usize = 0;
    let last_asset = match last_id {
        Some(last_id) => Some(
            asset::get_asset(pool, last_id)
                .await
                .wrap_err("no asset with id last_asset_id")?,
        ),
        None => None,
    };
    let maybe_group_of_last_asset: Option<TimelineGroupAlbum> = match last_id {
        Some(asset_id) => Some(
            album::get_timeline_group_album_for_asset(asset_id, pool.acquire().await?.as_mut())
                .await?,
        ),
        None => None,
    }
    .flatten();
    if last_asset.is_some() {
        // first check if the asset with last_id was displayed as part of a timeline group
        // if yes, serve the remaining assets of that group
        if let Some(group_album) = &maybe_group_of_last_asset {
            let assets_in_group: Vec<Asset> = sqlx::query_as!(
                DbAsset,
                r#"
WITH 
last_asset_date AS 
    (
    SELECT Asset.taken_date as date FROM Asset WHERE Asset.id = $1
    )
SELECT
Asset.id,
Asset.ty as "ty: _",
Asset.root_dir_id,
Asset.file_path,
Asset.file_type,
Asset.hash,
Asset.is_hidden,
Asset.added_at,
Asset.taken_date,
Asset.timezone_offset,
Asset.timezone_info as "timezone_info: _",
Asset.width,
Asset.height,
Asset.rotation_correction as "rotation_correction: _",
Asset.gps_latitude as "gps_latitude: _",
Asset.gps_longitude as "gps_longitude: _",
Asset.thumb_small_square_avif as "thumb_small_square_avif: _",
Asset.thumb_small_square_webp as "thumb_small_square_webp: _",
Asset.thumb_large_orig_avif as "thumb_large_orig_avif: _",
Asset.thumb_large_orig_webp as "thumb_large_orig_webp: _",
Asset.thumb_small_square_width,
Asset.thumb_small_square_height,
Asset.thumb_large_orig_width,
Asset.thumb_large_orig_height,
Asset.image_format_name,
Asset.video_codec_name,
Asset.video_bitrate,
Asset.audio_codec_name,
Asset.has_dash
FROM
Asset INNER JOIN AlbumEntry ON Asset.id = AlbumEntry.asset_id
WHERE
AlbumEntry.album_id = $2
AND (
    Asset.taken_date < (SELECT date FROM last_asset_date)
    OR (Asset.taken_date = (SELECT date FROM last_asset_date) AND Asset.id < $1)
)
ORDER BY Asset.taken_date DESC, Asset.id DESC
LIMIT $3;
        "#,
                last_id,
                group_album.album.id,
                max_count
            )
            // ^^ get assets in same group as asset last_id, with date either strictly smaller than last
            // asset or equal date and smaller id (fallback sorting key)
            .fetch_all(pool)
            .await
            .wrap_err("assets in group query failed")?
            .into_iter()
            .map(|db_asset| db_asset.try_into())
            .collect::<Result<Vec<_>>>()?;
            if !assets_in_group.is_empty() {
                // we have the album in the query above but ehh
                let album_id: AlbumId = sqlx::query!(
                    r#"
        SELECT Album.id FROM 
        Album INNER JOIN AlbumEntry ON AlbumEntry.album_id = Album.id
        WHERE AlbumEntry.asset_id = ? AND Album.is_timeline_group != 0;
        "#,
                    last_id
                )
                .fetch_one(pool)
                .await
                .map_err(DbError::from)
                .wrap_err("get group album for last asset query failed")?
                .id
                .into();
                // don't want to rewrite the conversion code to AlbumType right here
                let album = album::get_album(pool, album_id).await?;
                let timeline_group_album = match album {
                    AlbumType::TimelineGroup(tga) => tga,
                    AlbumType::Album(_) => {
                        error!(?album, "BUG: ended up with wrong album type!");
                        return Err(eyre!("BUG: ended up with wrong album type!"));
                    }
                };
                result_assets_size += assets_in_group.len();
                result.push(TimelineElement::Group {
                    group: timeline_group_album,
                    assets: assets_in_group,
                });
                if result_assets_size >= max_count.try_into()? {
                    return Ok(result);
                }
            }
        }
    }
    // now serve assets starting from either:
    //   date of group_of_last_asset if that is not None
    //   or date of last asset if group_of_last_asset is None
    let start_date = match &maybe_group_of_last_asset {
        Some(tga) => Some(tga.group.display_date),
        None => last_asset.map(|asset| asset.base.taken_date),
    };
    let start_from_beginning = start_date.is_none();
    let start_date_timestamp = datetime_to_db_repr(&start_date.unwrap_or(Utc::now())); // default value
                                                                                       // unused by query if start_from_beginning is true
    let last_asset_id_or_invalid_default = last_id.unwrap_or(AssetId(-1)); // can only be None if start_from_beginning is true
    let all_assets: Vec<Asset> = sqlx::query_as!(
        DbAsset,
        r#"
SELECT 
Asset.id,
Asset.ty as "ty: _",
Asset.root_dir_id,
Asset.file_path,
Asset.file_type,
Asset.hash,
Asset.is_hidden,
Asset.added_at,
Asset.taken_date,
Asset.timezone_offset,
Asset.timezone_info as "timezone_info: _",
Asset.width,
Asset.height,
Asset.rotation_correction as "rotation_correction: _",
Asset.gps_latitude as "gps_latitude: _",
Asset.gps_longitude as "gps_longitude: _",
Asset.thumb_small_square_avif as "thumb_small_square_avif: _",
Asset.thumb_small_square_webp as "thumb_small_square_webp: _",
Asset.thumb_large_orig_avif as "thumb_large_orig_avif: _",
Asset.thumb_large_orig_webp as "thumb_large_orig_webp: _",
Asset.thumb_small_square_width,
Asset.thumb_small_square_height,
Asset.thumb_large_orig_width,
Asset.thumb_large_orig_height,
Asset.image_format_name,
Asset.video_codec_name,
Asset.video_bitrate,
Asset.audio_codec_name,
Asset.has_dash
FROM Asset
WHERE Asset.taken_date < $1 OR (Asset.taken_date = $1 AND Asset.id < $2) OR $3
ORDER BY Asset.taken_date DESC, Asset.id DESC
LIMIT $4;
    "#,
        start_date_timestamp,
        last_asset_id_or_invalid_default,
        start_from_beginning,
        max_count
    )
    .fetch_all(pool)
    .await
    .wrap_err("could not query table Asset")?
    .into_iter()
    .map(|a| a.try_into())
    .collect::<Result<Vec<_>>>()?;

    let max_end_date_timestamp = match all_assets.last() {
        None => return Ok(result),
        Some(a) => a.base.taken_date.timestamp(),
    };

    // get timeline groups between first and last asset,
    // check their display date
    // if it's between start and end date
    // include whole group
    let groups_in_timespan: Vec<TimelineGroupAlbum> = sqlx::query_as!(
        DbAlbum,
        r#"
SELECT Album.*, NULL as "num_assets: _" FROM Album
WHERE Album.is_timeline_group != 0
AND Album.timeline_group_display_date > ?
AND Album.timeline_group_display_date <= ?
ORDER BY Album.timeline_group_display_date DESC, Album.id DESC;
    "#,
        start_date_timestamp,
        max_end_date_timestamp
    )
    .fetch_all(pool)
    .await
    .wrap_err("could not query table Album")?
    .into_iter()
    .map(|db_album| TimelineGroupAlbum::try_from(&db_album))
    .collect::<Result<Vec<_>>>()?;

    // now zip asset list with groups, inserting all assets in group at correct points.
    // always add whole groups so we don't have incomplete groups at the end of a chunk which
    // complicates the api.
    // Doing it this way, the earliest (oldest) date of the chunk is the correct date from which
    // to start assembling the next chunk assuming: no group diplay_date is before any of its asset
    // dates, only whole groups are returned
    // limit number of returned assets to count, or add last group in whole even if it goes over
    let mut group_iter = groups_in_timespan.into_iter().peekable();
    let mut next_group = group_iter.next();
    // collect assets belonging to the same day(for now, this is the only grouping key) in here
    let mut current_day_grouped: Vec<Asset> = Vec::default();
    for asset in &all_assets {
        if result_assets_size > max_count as usize {
            break;
        }
        match next_group {
            Some(ng) if ng.group.display_date < asset.base.taken_date => {
                // push the assets already grouped by date first
                if !current_day_grouped.is_empty() {
                    let d = std::mem::replace(&mut current_day_grouped, Vec::default());
                    result.push(TimelineElement::DayGrouped(d));
                }
                let mut group_assets = get_assets_in_album(ng.album.id, pool).await?;
                group_assets.sort_by_key(|a| a.base.taken_date);
                result_assets_size += group_assets.len();
                result.push(TimelineElement::Group {
                    group: ng.clone(),
                    assets: group_assets,
                });
                next_group = group_iter.next();
            }
            _ => {
                // don't need to push group album
                result_assets_size += 1;
                match current_day_grouped.last() {
                    None => {
                        current_day_grouped.push(asset.clone());
                    }
                    Some(date_grouped_asset) => {
                        let current_group_date =
                            date_grouped_asset.base.taken_date_local().date_naive();
                        let asset_date = asset.base.taken_date_local().date_naive();
                        if current_group_date == asset_date {
                            current_day_grouped.push(asset.clone());
                        } else {
                            // push current grouping and start new one
                            let d =
                                std::mem::replace(&mut current_day_grouped, vec![asset.clone()]);
                            result.push(TimelineElement::DayGrouped(d));
                        }
                    }
                }
            }
        };
    }
    if !current_day_grouped.is_empty() {
        result.push(TimelineElement::DayGrouped(current_day_grouped));
    }
    return Ok(result);
}
