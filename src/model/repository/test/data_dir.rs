use claims::{assert_err, assert_ok, assert_some};

use crate::model::{repository, DataDir, DataDirId};

use super::*;

#[tokio::test]
async fn insert_retrieve() {
    let pool = create_db().await;
    let data_dir = DataDir {
        id: DataDirId(0),
        path: "/path/to/data".into(),
    };
    let data_dir2 = DataDir {
        id: DataDirId(0),
        path: "/path/to/more/data".into(),
    };
    let data_dir_id = assert_ok!(repository::data_dir::insert_data_dir(&pool, &data_dir).await);
    let _data_dir2_id = assert_ok!(repository::data_dir::insert_data_dir(&pool, &data_dir2).await);
    let data_dir_with_id = DataDir {
        id: data_dir_id,
        ..data_dir
    };
    let retrieved = assert_ok!(repository::data_dir::get_data_dir(&pool, data_dir_id).await);
    assert_eq!(retrieved, data_dir_with_id);
}

#[tokio::test]
async fn get_by_path() {
    let pool = create_db().await;
    let data_dir = DataDir {
        id: DataDirId(0),
        path: "/path/to/data".into(),
    };
    let data_dir2 = DataDir {
        id: DataDirId(0),
        path: "/path/to/more/data".into(),
    };
    let _data_dir_id = assert_ok!(repository::data_dir::insert_data_dir(&pool, &data_dir).await);
    let data_dir2_id = assert_ok!(repository::data_dir::insert_data_dir(&pool, &data_dir2).await);
    let data_dir2_with_id = DataDir {
        id: data_dir2_id,
        ..data_dir2
    };
    let retrieved = assert_some!(assert_ok!(
        repository::data_dir::get_data_dir_with_path(&pool, "/path/to/more/data").await
    ));
    assert_eq!(retrieved, data_dir2_with_id);
}

#[allow(unused_must_use)]
#[tokio::test]
async fn insert_with_non_unique_path_fails() {
    let pool = create_db().await;
    let data_dir = DataDir {
        id: DataDirId(0),
        path: "/path/to/data".into(),
    };
    let data_dir2 = DataDir {
        id: DataDirId(0),
        path: "/path/to/data".into(),
    };
    let _data_dir_id = assert_ok!(repository::data_dir::insert_data_dir(&pool, &data_dir).await);
    assert_err!(repository::data_dir::insert_data_dir(&pool, &data_dir2).await);
}
