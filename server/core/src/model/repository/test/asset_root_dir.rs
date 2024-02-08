use camino::Utf8PathBuf as PathBuf;
use claims::{assert_err, assert_ok, assert_some};
use pretty_assertions::assert_eq;

use crate::model::{repository, AssetRootDir, AssetRootDirId};

#[test]
fn insert_retrieve() {
    let mut conn = super::db::open_in_memory_and_migrate();
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let asset_root_dir2 = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/more/assets"),
    };
    let root_dir_id = assert_ok!(repository::asset_root_dir::insert_asset_root(
        &mut conn,
        &asset_root_dir
    ));
    let root_dir2_id = assert_ok!(repository::asset_root_dir::insert_asset_root(
        &mut conn,
        &asset_root_dir2
    ));
    let root_dir_with_id = AssetRootDir {
        id: root_dir_id,
        ..asset_root_dir
    };
    let root_dir2_with_id = AssetRootDir {
        id: root_dir2_id,
        ..asset_root_dir2
    };
    let retrieved = assert_ok!(repository::asset_root_dir::get_asset_root(
        &mut conn,
        root_dir_id
    ));
    assert_eq!(retrieved, root_dir_with_id);
    let all_asset_root_dirs = assert_ok!(repository::asset_root_dir::get_asset_roots(&mut conn));
    assert!(all_asset_root_dirs.len() == 2);
    assert!(all_asset_root_dirs.contains(&root_dir_with_id));
    assert!(all_asset_root_dirs.contains(&root_dir2_with_id));
}

#[test]
fn get_by_path() {
    let mut conn = super::db::open_in_memory_and_migrate();
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let asset_root_dir2 = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/more/assets"),
    };
    let root_dir_id = assert_ok!(repository::asset_root_dir::insert_asset_root(
        &mut conn,
        &asset_root_dir
    ));
    let _root_dir2_id = assert_ok!(repository::asset_root_dir::insert_asset_root(
        &mut conn,
        &asset_root_dir2
    ));
    let root_dir_with_id = AssetRootDir {
        id: root_dir_id,
        ..asset_root_dir
    };
    let retrieved = assert_some!(assert_ok!(
        repository::asset_root_dir::get_asset_root_with_path(
            &mut conn,
            &PathBuf::from("/path/to/assets")
        )
    ));
    assert_eq!(retrieved, root_dir_with_id);
}

#[test]
fn inserting_with_non_unique_path_fails() {
    let mut conn = super::db::open_in_memory_and_migrate();
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let _root_dir_id = assert_ok!(repository::asset_root_dir::insert_asset_root(
        &mut conn,
        &asset_root_dir
    ));
    let asset_root_dir2 = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let _ = assert_err!(repository::asset_root_dir::insert_asset_root(
        &mut conn,
        &asset_root_dir2
    ));
}
