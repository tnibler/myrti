use std::collections::{HashMap, HashSet};

use camino::Utf8PathBuf as PathBuf;
use claims::assert_ok;
use itertools::Itertools;
use proptest::prelude::*;

use super::proptest_arb::{arb_new_album, arb_new_asset};
use super::util::{prop_insert_create_test_assets, set_assets_root_dir};

use crate::model::repository::album::AddItemToAlbum;
use crate::model::repository::test::proptest_arb::arb_new_album_item;
use crate::model::repository::test::util::prop_insert_create_test_asset;
use crate::model::{
    repository::{self, album::CreateAlbum},
    Album, Asset, AssetId, AssetRootDir, AssetRootDirId,
};
use crate::model::{AlbumItem, AlbumItemType, AssetBase};

#[test]
fn prop_create_retrieve_albums() {
    prop_compose! {
        fn arb_assets_and_albums()
        (
            items in prop::collection::vec(arb_new_album_item(), 0..20),
            albums in prop::collection::vec(arb_new_album(), 1..2)
        )
            (
                albums_asset_idxs in albums.into_iter().map(|album| (Just(album), prop::collection::vec(any::<prop::sample::Index>(), 0..=items.len()))).collect::<Vec<_>>(),
                items in Just(items),
            ) -> (Vec<AlbumItemType>, Vec<(Album, Vec<prop::sample::Index>)>) {
                (items, albums_asset_idxs)
            }
    }
    proptest!(|(
    (items, albums_asset_idxs) in arb_assets_and_albums(),
    append_chunk_size in 1usize..5)| {
        let mut conn = super::db::open_in_memory_and_migrate();
        let asset_root_dir = AssetRootDir {
            id: AssetRootDirId(0),
            path: PathBuf::from("/path/to/assets"),
        };
        let root_dir_id = assert_ok!(repository::asset_root_dir::insert_asset_root(
            &mut conn,
            &asset_root_dir
        ));
        // set root_dir_id for Asset items, insert into the db and set assset_id
        let items = items.into_iter().map(|item| match item {
            AlbumItemType::Asset(asset) => {
                let asset_to_insert = Asset {
                    base: AssetBase {
                        root_dir_id,
                        ..asset.base
                    },
                    ..asset
                };
                let asset_with_id = prop_insert_create_test_asset(&mut conn, &asset_to_insert)?;
                Ok(AlbumItemType::Asset(asset_with_id))
            },
            item @ AlbumItemType::Text(_) => Ok(item),
        }).collect::<Result<Vec<_>, TestCaseError>>()?;

        let mut albums_with_ids: Vec<Album> = Vec::default();
        for (album, _) in &albums_asset_idxs {
            let create_album = CreateAlbum {
                name: album.name.clone(),
                description: album.description.clone(),
            };
            // TODO initial creation with assets to insert right away not tested here
            let album_insert_result = repository::album::create_album(&mut conn, create_album, &[]);
            prop_assert!(album_insert_result.is_ok());
            let album_id = album_insert_result.unwrap();
            let album_with_id = Album { id: album_id, ..album.clone() };
            albums_with_ids.push(album_with_id);
        }
        let mut albums_items_with_ids: Vec<(Album, Vec<AlbumItemType>)> = Vec::default();
        for (album, (_album_no_id, asset_idxs)) in albums_with_ids.iter().zip(albums_asset_idxs.iter()) {
            // removing duplicates
            let assets_to_append: Vec<AlbumItemType> = asset_idxs.iter().map(|idx| idx.get(&items).clone()).collect::<HashSet<_>>().into_iter().collect();
            let mut assets_actually_appended: Vec<AlbumItemType> = Vec::default();
            let append_chunks: Vec<&[AlbumItemType]> = assets_to_append.chunks(append_chunk_size).collect();
            for chunk in append_chunks {
                let append_items = chunk.iter().map(|item| match item {
                    AlbumItemType::Asset(asset) => AddItemToAlbum::Asset(asset.base.id),
                    AlbumItemType::Text(text) => AddItemToAlbum::Text(text.clone()),
                }).collect_vec();
                let append_result = repository::album::append_items_to_album(&mut conn, album.id, &append_items);
                prop_assert!(append_result.is_ok());
                assets_actually_appended.extend_from_slice(chunk);
            }
            albums_items_with_ids.push((album.clone(), assets_actually_appended));
        }
        for (album, expected_items) in albums_items_with_ids {
            let retrieve_result = repository::album::get_items_in_album(&mut conn, album.id);
            prop_assert!(retrieve_result.is_ok());
            let actual_items_in_album: Vec<AlbumItem> = retrieve_result.unwrap();
            let expected_indices: Vec<usize> = (0..expected_items.len()).collect();
            prop_assert!(expected_items.len() == actual_items_in_album.len());
            actual_items_in_album.into_iter().zip(expected_items.into_iter()).map(|(actual, expected)| {
                match (actual.item, expected) {
                    (AlbumItemType::Asset(asset_actual), AlbumItemType::Asset(asset_expected)) => prop_assert!(asset_actual.base.id == asset_expected.base.id),
                    (AlbumItemType::Text(text_actual), AlbumItemType::Text(text_expected)) => prop_assert!(text_actual == text_expected),
                    _ => prop_assert!(false)
                }
                Ok(())
            }).collect::<Result<Vec<_>, _>>()?;
            let actual_indices: Vec<usize> = {
                use diesel::prelude::*;
                use super::super::schema::AlbumItem;
                AlbumItem::table
                    .filter(AlbumItem::album_id.eq(album.id.0))
                    .select(AlbumItem::idx)
                    .order_by(AlbumItem::idx)
                    .load(&mut conn).unwrap()
                }.into_iter().map(|i: i32| i.try_into()).collect::<Result<Vec<usize>, _>>()?;
            prop_assert_eq!(expected_indices, actual_indices);
        }
    })
}
