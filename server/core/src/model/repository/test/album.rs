use std::collections::{HashMap, HashSet};

use camino::Utf8PathBuf as PathBuf;
use claims::assert_ok;
use proptest::prelude::*;

use super::proptest_arb::{arb_new_album, arb_new_asset};
use super::util::{prop_insert_create_test_assets, set_assets_root_dir};

use crate::model::{
    repository::{self, album::CreateAlbum},
    Album, Asset, AssetId, AssetRootDir, AssetRootDirId,
};

#[test]
fn prop_create_retrieve_albums() {
    prop_compose! {
        fn arb_assets_and_albums()
        (
            assets in prop::collection::vec(arb_new_asset(), 1..5),
            albums in prop::collection::vec(arb_new_album(), 1..2)
        )
            (
                albums_asset_idxs in albums.into_iter().map(|album| (Just(album), prop::collection::vec(any::<prop::sample::Index>(), 0..assets.len()))).collect::<Vec<_>>(),
                assets in Just(assets),
            ) -> (Vec<Asset>, Vec<(Album, Vec<prop::sample::Index>)>) {
                (assets, albums_asset_idxs)
            }
    }
    proptest!(|(
    (assets, albums_asset_idxs) in arb_assets_and_albums(),
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
        let assets = set_assets_root_dir(assets, root_dir_id);

        let assets_with_ids: Vec<Asset> = prop_insert_create_test_assets(&mut conn, &assets)?;
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
        let mut albums_assets_with_ids: Vec<(Album, Vec<Asset>)> = Vec::default();
        let mut albums_by_asset: HashMap<AssetId, Vec<Album>> = HashMap::default();
        for (album, (_album_no_id, asset_idxs)) in albums_with_ids.iter().zip(albums_asset_idxs.iter()) {
            // removing duplicates
            let assets_to_append: Vec<Asset> = asset_idxs.iter().map(|idx| idx.get(&assets_with_ids).clone()).into_iter().collect::<HashSet<_>>().into_iter().collect();
            let mut assets_actually_appended: Vec<Asset> = Vec::default();
            let append_chunks: Vec<&[Asset]> = assets_to_append.chunks(append_chunk_size).collect();
            for chunk in append_chunks {
                let chunk_ids: Vec<AssetId> = chunk.into_iter().map(|asset| asset.base.id).collect();
                let append_result = repository::album::append_assets_to_album(&mut conn, album.id, &chunk_ids);
                // if any asset was already in a group album and the album we're inserting into
                // is also a group, adding to the album should fail
                prop_assert!(append_result.is_ok());
                assets_actually_appended.extend_from_slice(chunk);
                chunk_ids.iter().for_each(|asset_id| match albums_by_asset.get_mut(asset_id) {
                    None => {
                        albums_by_asset.insert(*asset_id, vec![album.clone()]);
                    },
                    Some(ref mut albums) => albums.push(album.clone())
                });
            }
            albums_assets_with_ids.push((album.clone(), assets_actually_appended));
        }
        for (album, expected_assets) in albums_assets_with_ids {
            let retrieve_result = repository::album::get_assets_in_album(&mut conn, album.id);
            prop_assert!(retrieve_result.is_ok());
            let actual_assets_in_album: Vec<Asset> = retrieve_result.unwrap();
            let expected_indices: Vec<usize> = (0..expected_assets.len()).collect();
            prop_assert_eq!(expected_assets, actual_assets_in_album);
            let actual_indices: Vec<usize> = {
                use diesel::prelude::*;
                use super::super::schema::AlbumEntry;
                AlbumEntry::table
                    .filter(AlbumEntry::album_id.eq(album.id.0))
                    .select(AlbumEntry::idx)
                    .order_by(AlbumEntry::idx)
                    .load(&mut conn).unwrap()
                }.into_iter().map(|i: i32| i.try_into()).collect::<Result<Vec<usize>, _>>()?;
            prop_assert_eq!(expected_indices, actual_indices);
        }
    })
}
