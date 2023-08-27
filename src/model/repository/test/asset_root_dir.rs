use std::path::PathBuf;

use claims::{assert_err, assert_ok, assert_some};
use pretty_assertions::assert_eq;

use crate::model::{repository, AssetRootDir, AssetRootDirId};

use super::*;

#[tokio::test]
async fn insert_retrieve() {
    let pool = create_db().await;
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let asset_root_dir2 = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/more/assets"),
    };
    let root_dir_id =
        assert_ok!(repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir).await);
    let root_dir2_id =
        assert_ok!(repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir2).await);
    let root_dir_with_id = AssetRootDir {
        id: root_dir_id,
        ..asset_root_dir
    };
    let root_dir2_with_id = AssetRootDir {
        id: root_dir2_id,
        ..asset_root_dir2
    };
    let retrieved =
        assert_ok!(repository::asset_root_dir::get_asset_root(&pool, root_dir_id).await);
    assert_eq!(retrieved, root_dir_with_id);
    let all_asset_root_dirs = assert_ok!(repository::asset_root_dir::get_asset_roots(&pool).await);
    assert!(all_asset_root_dirs.len() == 2);
    assert!(all_asset_root_dirs.contains(&root_dir_with_id));
    assert!(all_asset_root_dirs.contains(&root_dir2_with_id));
}

#[tokio::test]
async fn get_by_path() {
    let pool = create_db().await;
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let asset_root_dir2 = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/more/assets"),
    };
    let root_dir_id =
        assert_ok!(repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir).await);
    let _root_dir2_id =
        assert_ok!(repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir2).await);
    let root_dir_with_id = AssetRootDir {
        id: root_dir_id,
        ..asset_root_dir
    };
    let retrieved = assert_some!(assert_ok!(
        repository::asset_root_dir::get_asset_root_with_path(
            &pool,
            &PathBuf::from("/path/to/assets")
        )
        .await
    ));
    assert_eq!(retrieved, root_dir_with_id);
}

#[allow(unused_must_use)]
#[tokio::test]
async fn inserting_with_non_unique_path_fails() {
    let pool = create_db().await;
    let asset_root_dir = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    let _root_dir_id =
        assert_ok!(repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir).await);
    let asset_root_dir2 = AssetRootDir {
        id: AssetRootDirId(0),
        path: PathBuf::from("/path/to/assets"),
    };
    assert_err!(repository::asset_root_dir::insert_asset_root(&pool, &asset_root_dir2).await);
}
