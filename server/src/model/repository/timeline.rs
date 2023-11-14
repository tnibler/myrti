use chrono::{DateTime, Utc};
use eyre::{Context, Result};

use crate::model::{
    repository::db_entity::{DbAlbum, DbAsset},
    Asset, TimelineGroupAlbum,
};

use super::{album::get_assets_in_album, pool::DbPool};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TimelineElement {
    DayGrouped(Vec<Asset>),
    Group {
        group: TimelineGroupAlbum,
        assets: Vec<Asset>,
    },
}

/// Query next chunk of asset timeline, starting at >start_date
/// TODO add id of last asset as parma in case to dates are equal to start_date+1
pub async fn get_timeline_chunk(
    pool: &DbPool,
    start_date: &DateTime<Utc>,
    count: i64,
) -> Result<Vec<TimelineElement>> {
    // get assets starting at start_date, limit count
    //
    // get timeline groups between first and last asset,
    // check their display date
    // if it's between start and end date
    // include whole group
    let start_date_timestamp = start_date.timestamp();
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
WHERE Asset.taken_date > ?
ORDER BY Asset.taken_date DESC, Asset.id DESC
LIMIT ?;
    "#,
        start_date_timestamp,
        count
    )
    .fetch_all(pool)
    .await
    .wrap_err("could not query table Asset")?
    .into_iter()
    .map(|a| a.try_into())
    .collect::<Result<Vec<_>>>()?;
    // date of last of all assets, which is >= the date of last asset in this chunk
    // (after groups are sorted out)
    let max_end_date_timestamp = match all_assets.last() {
        None => return Ok(Vec::default()),
        Some(a) => a.base.taken_date.timestamp(),
    };
    let groups_in_timespan: Vec<TimelineGroupAlbum> = sqlx::query_as!(
        DbAlbum,
        r#"
SELECT Album.* FROM Album
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
    let mut chunk: Vec<TimelineElement> = Vec::default();
    let mut group_iter = groups_in_timespan.into_iter().peekable();
    let mut chunk_size: usize = 0;
    let mut next_group = group_iter.next();
    // collect assets belonging to the same day(for now, this is the only grouping key) in here
    let mut current_day_grouped: Vec<Asset> = Vec::default();
    for asset in &all_assets {
        if chunk_size > count as usize {
            break;
        }
        match next_group {
            Some(ng) if ng.group.display_date < asset.base.taken_date => {
                // push the assets already grouped by date first
                if !current_day_grouped.is_empty() {
                    let d = std::mem::replace(&mut current_day_grouped, Vec::default());
                    chunk.push(TimelineElement::DayGrouped(d));
                }
                let mut group_assets = get_assets_in_album(ng.album.id, pool).await?;
                group_assets.sort_by_key(|a| a.base.taken_date);
                chunk_size += group_assets.len();
                chunk.push(TimelineElement::Group {
                    group: ng.clone(),
                    assets: group_assets,
                });
                next_group = group_iter.next();
            }
            _ => {
                // don't need to push group album
                chunk_size += 1;
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
                            chunk.push(TimelineElement::DayGrouped(d));
                        }
                    }
                }
            }
        };
    }
    if !current_day_grouped.is_empty() {
        chunk.push(TimelineElement::DayGrouped(current_day_grouped));
    }
    return Ok(chunk);
}
