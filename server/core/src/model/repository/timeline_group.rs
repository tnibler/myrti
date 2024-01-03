use chrono::{DateTime, Utc};
use eyre::{Context, Result};
use sqlx::SqliteConnection;

use crate::model::{
    repository::db_entity::{DbAsset, DbTimelineGroup},
    timeline_group::TimelineGroup,
    util::datetime_to_db_repr,
    Asset, AssetId, TimelineGroupEntryId, TimelineGroupId,
};

use super::pool::DbPool;

pub async fn get_timeline_group(pool: &DbPool, id: TimelineGroupId) -> Result<TimelineGroup> {
    sqlx::query_as!(
        DbTimelineGroup,
        r#"
    SELECT * FROM TimelineGroup WHERE id = ?;
    "#,
        id
    )
    .fetch_one(pool)
    .await
    .wrap_err("error getting TimelineGroup row by id")?
    .try_into()
}

pub async fn get_timeline_group_album_for_asset(
    asset_id: AssetId,
    conn: &mut SqliteConnection,
) -> Result<Option<TimelineGroup>> {
    sqlx::query_as!(
        DbTimelineGroup,
        r#"
SELECT TimelineGroup.* FROM
TimelineGroup INNER JOIN TimelineGroupEntry
ON TimelineGroupEntry.group_id = TimelineGroup.id
WHERE TimelineGroupEntry.asset_id = ?;
    "#,
        asset_id
    )
    .fetch_optional(&mut *conn)
    .await
    .wrap_err("get timeline group for asset: could not query optional single row from table Album")?
    .map(|db_tg| db_tg.try_into())
    .transpose()
    // .wrap_err("could not convert DbTimelineGroup to model TimelineGroup")
}

pub struct CreateTimelineGroup {
    pub name: Option<String>,
    pub display_date: DateTime<Utc>,
    pub asset_ids: Vec<AssetId>,
}

pub async fn create_timeline_group(
    pool: &DbPool,
    ctg: CreateTimelineGroup,
) -> Result<TimelineGroupId> {
    let mut tx = pool.begin().await?;
    let now = Utc::now();
    let db_display_date = datetime_to_db_repr(&ctg.display_date);
    let db_now = datetime_to_db_repr(&now);
    let result = sqlx::query!(
        r#"
INSERT INTO TimelineGroup
(id, name, display_date, created_at, changed_at) 
VALUES
(NULL, ?, ?, ?, ?);
    "#,
        ctg.name,
        db_display_date,
        db_now,
        db_now
    )
    .execute(tx.as_mut())
    .await?;

    let group_id = TimelineGroupId(result.last_insert_rowid());

    if !ctg.asset_ids.is_empty() {
        let insert_tuples = ctg.asset_ids.into_iter();
        let mut query_builder = sqlx::QueryBuilder::new(
            r#"
INSERT INTO TimelineGroupEntry(id, group_id, asset_id)
    "#,
        );
        query_builder.push_values(insert_tuples, move |mut builder, asset_id| {
            builder.push_bind(None::<TimelineGroupEntryId>);
            builder.push_bind(group_id);
            builder.push_bind(asset_id);
        });
        query_builder.push(r#";"#);
        query_builder
            .build()
            .execute(&mut *tx)
            .await
            .wrap_err("could not insert into table TimelineGroupEntry")?;
    }

    tx.commit().await?;
    Ok(group_id)
}

pub async fn add_assets_to_group(
    tx: &mut SqliteConnection,
    group_id: TimelineGroupId,
    asset_ids: &[AssetId],
) -> Result<()> {
    if asset_ids.is_empty() {
        return Ok(());
    }
    let mut query_builder = sqlx::QueryBuilder::new(
        r#"
INSERT INTO TimelineGroupEntry(id, group_id, asset_id)
    "#,
    );
    query_builder.push_values(asset_ids.iter(), move |mut builder, asset_id| {
        builder.push_bind(None::<TimelineGroupEntryId>);
        builder.push_bind(group_id);
        builder.push_bind(asset_id);
    });
    query_builder.push(r#";"#);
    query_builder
        .build()
        .execute(&mut *tx)
        .await
        .wrap_err("could not insert into table TimelineGroupEntry")?;
    Ok(())
}

pub async fn get_assets_in_group(pool: &DbPool, group_id: TimelineGroupId) -> Result<Vec<Asset>> {
    sqlx::query_as!(
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
    FROM Asset, TimelineGroup, TimelineGroupEntry
    WHERE Asset.id = TimelineGroupEntry.asset_id
    AND TimelineGroupEntry.group_id = TimelineGroup.id
    AND TimelineGroup.id = ?
    ORDER BY Asset.taken_date;
    "#,
        group_id
    )
    .fetch_all(pool)
    .await
    .wrap_err("could not query Assets in TimelineGroup")?
    .into_iter()
    .map(|db_asset| db_asset.try_into())
    .collect::<Result<Vec<_>>>()
}
