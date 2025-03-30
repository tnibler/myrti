use std::collections::HashSet;

use camino::Utf8PathBuf as PathBuf;
use claims::assert_ok;
use is_sorted::IsSorted;

use proptest::prelude::*;

use crate::model::{
    repository, repository::timeline::TimelineElement,  Asset, AssetId,
    AssetRootDir, AssetRootDirId, TimelineGroup,
    repository::db::DbConn,
};
use proptest_arb::{arb_new_timeline_group, arb_new_asset};

use super::{util::*, *};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GroupWithAssets {
    pub group: TimelineGroup,
    pub assets: Vec<Asset>,
}


#[test]
fn prop_test_timeline() {
    // generate assets
    // pick random distinct subsets
    // create groups from subsets, assigning random display_dates
    // query timeline in different chunk sizes
    //
    // invariants in incresing order of complexity:
    //  - no duplicate assets between chunks
    //  - date ordering
    //  - all assets contained in chunks
    //  - chunks are correct
    prop_compose! {
        fn arb_timeline_group_with_assets()
        (
            group in arb_new_timeline_group(),
            assets in prop::collection::vec(arb_new_asset(), 1..30)
        ) -> GroupWithAssets {
            GroupWithAssets { group, assets }
        }
    }
    proptest!(|(
        assets_not_in_groups in prop::collection::vec(arb_new_asset(), 0..100),
        groups_with_assets in prop::collection::vec(arb_timeline_group_with_assets(), 0..50),
        timeline_chunk_size in 1..100usize,
    )| {
        let local_tz = &chrono::Local;
        let mut dbgstr = String::new();

        let mut conn = super::db::open_in_memory_and_migrate();
        let asset_root_dir = AssetRootDir {
            id: AssetRootDirId(0),
            path: PathBuf::from("/path/to/assets"),
        };
        let root_dir_id = assert_ok!(repository::asset_root_dir::insert_asset_root(&mut conn, &asset_root_dir));
        // set root_dir_id for all assets
        let assets_not_in_groups = set_assets_root_dir(assets_not_in_groups, root_dir_id);
        let groups_with_assets: Vec<GroupWithAssets> = groups_with_assets.into_iter().map(|gwa| {
            GroupWithAssets {
                assets: set_assets_root_dir(gwa.assets, root_dir_id),
                group: gwa.group,
            }
        }).collect();

        // assets and groups with ids set
        let assets_not_in_groups: Vec<Asset> = prop_insert_create_test_assets(&mut conn, &assets_not_in_groups)?;
        let groups_with_assets: Vec<GroupWithAssets> = prop_insert_timeline_groups_insert_add_assets(&mut conn, &groups_with_assets)?;
        let expected_all_assets: HashSet<Asset> = assets_not_in_groups.iter().chain(groups_with_assets.iter().flat_map(|gwa| &gwa.assets)).cloned().collect();
        let actual_all_assets_in_db: HashSet<Asset> = repository::asset::get_assets(&mut conn).unwrap().into_iter().collect();
        prop_assert_eq!(&expected_all_assets, &actual_all_assets_in_db, "Setup went wrong: not all assets in db");
        let expected_num_chunks =  expected_all_assets.len().div_ceil(timeline_chunk_size);

        dbgstr.push_str("\nALL ASSETS\n");
        for a in &assets_not_in_groups {
            dbgstr.push_str(&format!("{} {} {:?} {:?}\n", a.base.id, a.base.taken_date, a.base.timestamp_info, a.base.taken_date.with_timezone(local_tz)));
        }
        for gwa in &groups_with_assets {
            dbgstr.push_str(&format!("{} {}\n", gwa.group.id, gwa.group.display_date));
            for a in &gwa.assets {
                dbgstr.push_str(&format!("\t{} {} {:?}\n", a.base.id, a.base.taken_date, a.base.timestamp_info));
            }
        }
        dbgstr.push_str("/ALL ASSETS\n\n");

        let mut last_id: Option<AssetId> = None;
        let mut chunks = Vec::default();
        for chunk_idx in 0..expected_num_chunks {
            // prop_assert!(chunk_idx < expected_num_chunks);
            dbgstr.push_str("CHUNK\n");
            let chunk = {
                let c = repository::timeline::get_timeline_chunk(&mut conn, last_id, timeline_chunk_size as i64);
                prop_assert!(c.is_ok(), "get_timeline_chunk returned error: \n{:?}", c.unwrap_err());
                c.unwrap()
            };
            prop_assert!(!chunk.is_empty(), "chunk at index {} is empty {}", chunk_idx, &dbgstr);
            for timeline_element in &chunk {
                match timeline_element {
                    TimelineElement::DayGrouped(assets) => {
                        dbgstr.push_str("Day group:\n");
                        for a in assets {
                            dbgstr.push_str(&format!("\t{} {}\n", a.base.id, a.base.taken_date));
                        }
                    }
                    TimelineElement::Group { group, assets} => {
                        dbgstr.push_str(&format!("Alb group({}): {}\n", group.id, group.display_date));
                        for a in assets {
                            dbgstr.push_str(&format!("\t{} {} {:?}\n", a.base.id, a.base.taken_date, a.base.taken_date.with_timezone(local_tz)));
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
        let next_chunk = {
            let c = repository::timeline::get_timeline_chunk(&mut conn, last_id, timeline_chunk_size as i64);
            prop_assert!(c.is_ok(), "get_timeline_chunk error:\n{}", c.unwrap_err());
            c.unwrap() 
        };
        prop_assert!(next_chunk.is_empty());

        // check number of non-empty chunks matches expected
        let actual_num_chunks = chunks.len();
        prop_assert_eq!(expected_num_chunks, actual_num_chunks);
        // check we got all assets
        let actual_all_assets: HashSet<Asset> = chunks.iter().flat_map(|chunk| chunk.iter().flat_map(|tl_el| tl_el.get_assets().iter())).cloned().collect();
        prop_assert_eq!(expected_all_assets, actual_all_assets);

        // check TimelineElements are ordered chronologically:
        //  - Assets in DayGrouped are ordered
        //  - Asset in Group are ordered
        //  - Dates of TimelineElement are descending
        let mut last_timeline_element: Option<&TimelineElement> = None;
        // A single DayGrouped may be split accross multiple chunks, in which case the two
        // resulting DayGrouped are allowed to have the same date (<=).
        // Within a chunk, dates should be strictly decreasing (<)
        let mut crossed_chunk_boundary: bool;
        for chunk in &chunks {
            crossed_chunk_boundary = true;
            for tlel in chunk {
                // verify order of assets in tlel
                match tlel {
                    TimelineElement::DayGrouped(assets) => {
                        prop_assert!(!assets.is_empty());
                        let date = assets[0].base.taken_date.with_timezone(local_tz).date_naive();
                        // check assets belong to same day
                        let all_dates_equal = assets.iter().all(|asset| asset.base.taken_date.with_timezone(local_tz).date_naive() == date);
                        prop_assert!(all_dates_equal);
                        // check asset dates are sorted descending
                        let assets_are_sorted = assets.iter().rev().is_sorted_by_key(|asset| (asset.base.taken_date, asset.base.id));
                        prop_assert!(assets_are_sorted);
                    }
                    TimelineElement::Group { group: _, assets } => {
                        // check asset dates are sorted descending
                        let assets_are_sorted = assets.iter().rev().is_sorted_by_key(|asset| (asset.base.taken_date, asset.base.id));
                        prop_assert!(assets_are_sorted);
                    }
                }
                let last_el = match last_timeline_element {
                    None => { 
                        last_timeline_element = Some(tlel); 
                        continue; 
                    }
                    Some(last_el) => last_el
                };
                // verify order of TimelineElements
                match (last_el, tlel) {
                    (TimelineElement::Group { group: last_group, assets: _}, TimelineElement::Group { group, assets: _}) => {
                        let last_group_date = last_group.display_date;
                        let last_group_id = last_group.id;
                        let cur_group_date = group.display_date;
                        let cur_group_id = group.id;
                        // multiple groups can have the same display_date so less or equal here
                        prop_assert!((cur_group_date, cur_group_id) <= (last_group_date, last_group_id), "{}", dbgstr);
                    }
                    (TimelineElement::Group { group, assets: _}, TimelineElement::DayGrouped(day_assets)) => {
                        let day_date = day_assets[0].base.taken_date.with_timezone(local_tz).date_naive();
                        let last_group_date = group.display_date.date_naive();
                        // less or equal because groups come before days in case of equality
                        prop_assert!(day_date <= last_group_date);
                    }
                    (TimelineElement::DayGrouped(last_assets), TimelineElement::DayGrouped(assets)) => {
                        let cur_date = assets[0].base.taken_date.with_timezone(local_tz).date_naive();
                        let last_date = last_assets[0].base.taken_date.with_timezone(local_tz).date_naive();
                        if crossed_chunk_boundary {
                            prop_assert!(cur_date <= last_date, "DayGrouped not in order: {} not <= {}\n {}", cur_date, last_date, dbgstr);
                        } else {
                            prop_assert!(cur_date < last_date, "DayGrouped not in order: {} not < {}\n {}", cur_date, last_date, dbgstr);
                        }
                    }
                    (TimelineElement::DayGrouped(last_assets), TimelineElement::Group { group, assets: _ }) => {
                        let cur_group_date = group.display_date;
                        let cur_group_date_day = cur_group_date.date_naive();
                        let last_date_day = last_assets[0].base.taken_date.with_timezone(local_tz).date_naive();
                        prop_assert!(cur_group_date_day <= last_date_day, "group date not <= last day date {}\n {}", group.display_date, dbgstr);
                        let last_asset_date = last_assets.last().unwrap().base.taken_date;
                        prop_assert!(cur_group_date < last_asset_date, "group date not < last asset date {}\n{}", group.display_date, dbgstr);
                    }
                }
                last_timeline_element = Some(tlel);
                crossed_chunk_boundary = false;
            }
        }
    });
}

fn prop_insert_timeline_groups_insert_add_assets(
    conn: &mut DbConn,
    groups: &[GroupWithAssets],
) -> Result<Vec<GroupWithAssets>, TestCaseError> {
    let mut groups_with_ids: Vec<GroupWithAssets> = Vec::default();
    for group in groups {
        let assets_with_id = prop_insert_create_test_assets(conn, &group.assets)?;
        let group_with_id = prop_insert_timeline_group_add_assets(
            conn,
            &group.group.clone(),
            assets_with_id.iter().map(|asset| asset.base.id),
        )
        ?;
        groups_with_ids.push(GroupWithAssets {
            assets: assets_with_id,
            group: group_with_id,
        });
    }
    Ok(groups_with_ids)
}
