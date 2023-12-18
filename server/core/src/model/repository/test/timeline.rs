use std::collections::HashSet;

use camino::Utf8PathBuf as PathBuf;
use claims::assert_ok;

use proptest::prelude::*;

use crate::model::{
    repository, repository::timeline::TimelineElement, Album, AlbumType, Asset, AssetId,
    AssetRootDir, AssetRootDirId, TimelineGroupAlbum,
};
use proptest_arb::{arb_new_album_timeline_group, arb_new_asset};

use super::{util::*, *};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GroupWithAssets {
    pub group: TimelineGroupAlbum,
    pub assets: Vec<Asset>,
}

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
        fn arb_timeline_album_with_assets(root_dir_id: AssetRootDirId)
        (
            album in arb_new_album_timeline_group(),
            assets in prop::collection::vec(arb_new_asset(root_dir_id), 1..5)
        ) -> GroupWithAssets {
            GroupWithAssets { group: album, assets }
        }
    }
    proptest!(|(
        assets_not_in_groups in prop::collection::vec(arb_new_asset(root_dir_id), 1..5),
        groups_with_assets in prop::collection::vec(arb_timeline_album_with_assets(root_dir_id), 0..3),
        timeline_chunk_size in 1..3usize,
    )| {
        let _ = rt.block_on(async {
            let mut dbgstr = String::new();
            sqlx::query!(r#"DELETE FROM AlbumEntry; DELETE FROM Asset; DELETE FROM Album; "#).execute(&pool).await.unwrap();
            // assets and albums with ids set
            let assets_not_in_groups: Vec<Asset> = prop_insert_create_test_assets(&pool, &assets_not_in_groups).await?;
            let groups_with_assets: Vec<GroupWithAssets> = prop_insert_group_albums_insert_add_assets(&pool, &groups_with_assets).await?;
            let expected_all_assets: HashSet<Asset> = assets_not_in_groups.iter().chain(groups_with_assets.iter().map(|gwa| &gwa.assets).flatten()).cloned().collect();
            let actual_all_assets_in_db: HashSet<Asset> = repository::asset::get_assets(&pool).await.unwrap().into_iter().collect();
            prop_assert_eq!(&expected_all_assets, &actual_all_assets_in_db, "Setup went wrong: not all assets in db");
            let expected_num_chunks =  expected_all_assets.len().div_ceil(timeline_chunk_size);

            dbgstr.push_str("\nALL ASSETS\n");
            for a in &assets_not_in_groups {
                dbgstr.push_str(&format!("{} {}\n", a.base.id, a.base.taken_date));
            }
            for gwa in &groups_with_assets {
                dbgstr.push_str(&format!("{} {}\n", gwa.group.album.id, gwa.group.group.display_date));
                for a in &gwa.assets {
                    dbgstr.push_str(&format!("\t{} {}\n", a.base.id, a.base.taken_date));
                }
            }
            dbgstr.push_str("/ALL ASSETS\n\n");

            let mut last_id: Option<AssetId> = None;
            let mut chunks = Vec::default();
            for chunk_idx in 0..expected_num_chunks {
                dbgstr.push_str("CHUNK\n");
                let chunk = {
                    let c = repository::timeline::get_timeline_chunk(&pool, last_id, timeline_chunk_size as i64).await;
                    prop_assert!(c.is_ok(), "get_timeline_chunk returned error: \n{:?}", c.unwrap_err());
                    c.unwrap()
                };
                prop_assert!(!chunk.is_empty(), "chunk at index {} is empty {}", chunk_idx, &dbgstr);
                for timeline_element in &chunk {
                    match timeline_element {
                        TimelineElement::DayGrouped(assets) => {
                            dbgstr.push_str(&"Day group:\n");
                            for a in assets {
                                dbgstr.push_str(&format!("\t{} {}\n", a.base.id, a.base.taken_date));
                            }
                        }
                        TimelineElement::Group { group, assets} => {
                            dbgstr.push_str(&format!("Alb group: {}\n", group.group.display_date));
                            for a in assets {
                                dbgstr.push_str(&format!("\t{} {}\n", a.base.id, a.base.taken_date));
                            }
                        }
                    }
                    let assets = timeline_element.get_assets();
                    prop_assert!(!assets.is_empty());
                }
                let num_assets_in_chunk: usize = chunk.iter().map(|c| c.get_assets().len()).sum();
                prop_assert!(num_assets_in_chunk <= timeline_chunk_size, "Returned chunk is too large ({} assets, max is {})", num_assets_in_chunk, timeline_chunk_size);
                let last_assets = chunk.last().unwrap().get_assets();
                last_id = Some(last_assets.last().unwrap().base.id);
                chunks.push(chunk);
            }
            let actual_num_chunks = chunks.len();
            prop_assert_eq!(expected_num_chunks, actual_num_chunks);
            let actual_all_assets: HashSet<Asset> = chunks.iter().map(|chunk| chunk.iter().map(|tl_el| tl_el.get_assets().iter()).flatten()).flatten().cloned().collect();
            prop_assert_eq!(expected_all_assets, actual_all_assets);
            let next_chunk = {
                let c = repository::timeline::get_timeline_chunk(&pool, last_id, timeline_chunk_size as i64).await;
                prop_assert!(c.is_ok());
                c.unwrap() 
            };
            prop_assert!(next_chunk.is_empty());
            Ok(())
        })?;
    });
}

async fn prop_insert_group_albums_insert_add_assets(
    pool: &DbPool,
    groups: &[GroupWithAssets],
) -> Result<Vec<GroupWithAssets>, TestCaseError> {
    let mut groups_with_ids: Vec<GroupWithAssets> = Vec::default();
    for group in groups {
        let assets_with_id = prop_insert_create_test_assets(pool, &group.assets).await?;
        let album_with_id = prop_insert_album_add_assets(
            pool,
            &AlbumType::TimelineGroup(group.group.clone()),
            assets_with_id.iter().map(|asset| asset.base.id),
        )
        .await?;
        groups_with_ids.push(GroupWithAssets {
            assets: assets_with_id,
            group: match album_with_id {
                AlbumType::TimelineGroup(tg) => tg,
                _ => unreachable!("wrong album type!"),
            },
        });
    }
    Ok(groups_with_ids)
}
