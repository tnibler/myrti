use std::assert_matches;
use std::sync::{Arc, Mutex};

use axum::http::header;
use axum::{
    Router,
    body::Body,
    http::{Response, StatusCode},
};
use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
use camino_tempfile_ext::prelude::*;
use chrono::DateTime;
use claims::{assert_err, assert_some_eq};
use eyre::Result;

use http_body_util::BodyExt;
use itertools::Itertools;
use myrti_core::{
    config::{AssetDir, BinPaths, Config, DataDir},
    core::{
        scheduler::{MessageFromScheduler, SchedulerHandle, SchedulerMessage, UserRequest},
        storage::Storage,
    },
    globset::Glob,
};
use myrti_data::model::{self};
use myrti_data::{
    db::{DbConn, DbPool},
    interact,
    model::AssetRootDirId,
    repository,
};
use serde::{Serialize, de::DeserializeOwned};
use tokio::process::Command;
use tokio::sync::broadcast::{self, error::RecvError};
use tower::{Service, ServiceExt};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::routes::asset::{SetAssetIsSeriesSelectionResponse, SetAssetSeriesSelectionRequest};
use crate::routes::timeline::{SegmentType, TimelineMonthSlice};
use crate::routes::timeline_group::{
    CreateTimelineGroupRequest, CreateTimelineGroupResponse, EditTimelineGroup,
    EditTimelineGroupRequest,
};
use crate::schema::asset::{AssetSpe, AssetWithSpe};
use crate::schema::{AssetId, TimelineGroupId};
use crate::{
    routes::timeline::{
        TimelineItem, TimelineSectionsResponse, TimelineSegmentsResponse, TimelineSegmentsWithId,
    },
    schema::AssetSeriesId,
    server::{Server, SetupConfig, make_app},
};

fn setup_tracing() {
    let tracing = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing_subscriber::filter::LevelFilter::DEBUG.into())
                .with_env_var("MYRTI_LOG")
                .from_env_lossy(),
        )
        .with(tracing_error::ErrorLayer::default())
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_target(true)
                .with_file(false)
                .with_line_number(false)
                .with_test_writer(),
        );
    _ = tracing.try_init();
}

pub struct Test {
    pub app: Router<()>,
    pub scheduler: SchedulerHandle,
    pub scheduler_recv: broadcast::Receiver<MessageFromScheduler>,
    pub db_pool: DbPool,
    pub config: Arc<Mutex<Config>>,
    pub asset_dir: Utf8TempDir,
}

impl Test {
    pub async fn new() -> Self {
        let orig_dir = if std::env::var("MYRTI_TEST_REAL_DATA").is_ok_and(|s| !s.is_empty()) {
            PathBuf::from("../test-data/library")
        } else {
            PathBuf::from("../test-data/fake-tree")
        };
        let asset_dir = tokio::task::spawn_blocking(|| camino_tempfile::tempdir().unwrap())
            .await
            .expect("directory setup panicked");

        let mut cp = Command::new("cp");
        cp.arg(orig_dir.join("."))
            .arg(asset_dir.path())
            .args(["-a", "-r"]);
        cp.spawn()
            .expect("error calling cp")
            .wait_with_output()
            .await
            .expect("cp exited with an error");

        let config = Config {
            asset_dirs: vec![AssetDir {
                name: None,
                path: asset_dir.path().to_path_buf(),
                exclude_globs: Default::default(),
                include_globs: Default::default(),
            }],
            data_dir: DataDir {
                path: PathBuf::from("data_directory"),
                name: None,
                db_path: None,
            },
            bin_paths: BinPaths {
                exiftool: Some((
                    PathBuf::from("../scripts/test-shims/exiftool.sh"),
                    vec![asset_dir.path().to_owned().into()],
                )),
                ffmpeg: Some((PathBuf::from("../scripts/test-shims/ffmpeg.sh"), vec![])),
                ffprobe: Some((
                    PathBuf::from("../scripts/test-shims/ffprobe.sh"),
                    vec![asset_dir.path().to_owned().into()],
                )),
                gpac: Some((PathBuf::from("../scripts/test-shims/gpac.sh"), vec![])),
            },
            address: None,
            port: None,
        };
        let storage = Storage::new(config.data_dir.path.clone());
        let config: Arc<_> = Mutex::new(config).into();

        let Server {
            app,
            scheduler,
            scheduler_recv,
            db_pool,
            ..
        } = make_app(SetupConfig {
            config: config.clone(),
            config_dir: "config_directory".into(),
            db_url: ":memory:",
            storage,
        })
        .await
        .expect("error creating app");

        Self {
            app,
            scheduler,
            scheduler_recv,
            db_pool,
            config,
            asset_dir,
        }
    }

    pub async fn send_scheduler(&mut self, msg: SchedulerMessage) {
        self.scheduler.send.send(msg).await.unwrap();
    }

    pub async fn wait_for_message(&mut self, want_msg: MessageFromScheduler) {
        match self.scheduler_recv.recv().await {
            Ok(msg) if msg == want_msg => {}
            other => panic!("unexpected message received: {:?}", other),
        }
    }

    pub async fn reindex_wait(&mut self) {
        self.scheduler
            .send
            .send(SchedulerMessage::UserRequest(
                UserRequest::ReindexAssetRoot(AssetRootDirId(1)),
            ))
            .await
            .unwrap();

        match self.scheduler_recv.recv().await {
            Ok(MessageFromScheduler::IndexingFinished(_)) => {}
            other => panic!("unexpected message received: {:?}", other),
        }
    }

    pub fn set_include(&mut self, globs: &[&str]) {
        let globs: Vec<_> = globs.iter().cloned().map(Glob::new).try_collect().unwrap();
        self.config.lock().unwrap().asset_dirs[0].include_globs = globs;
    }

    pub fn set_exclude(&mut self, globs: &[&str]) {
        let globs: Vec<_> = globs.iter().cloned().map(Glob::new).try_collect().unwrap();
        self.config.lock().unwrap().asset_dirs[0].exclude_globs = globs;
    }

    pub fn add_includes(&mut self, globs: &[&str]) {
        let globs: Vec<_> = globs.iter().cloned().map(Glob::new).try_collect().unwrap();
        self.config.lock().unwrap().asset_dirs[0]
            .include_globs
            .extend(globs)
    }

    pub async fn db_query<F, T>(&mut self, func: F) -> Result<T>
    where
        F: FnOnce(&mut DbConn) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let conn = self.db_pool.get().await.unwrap();
        interact!(conn, move |conn| { func(conn) }).await.unwrap()
    }

    pub async fn asset_with_path(&mut self, path: impl AsRef<Path>) -> model::Asset {
        let all_assets = self.db_query(repository::asset::get_assets).await.unwrap();
        all_assets
            .into_iter()
            .find(|asset| asset.rep_file.file_path == path.as_ref())
            .unwrap_or_else(|| panic!("no AssetFile with this filename found: {}", path.as_ref()))
    }
}

#[tokio::test]
async fn timeline_group_basic() {
    setup_tracing();
    let mut test = Test::new().await;

    test.set_exclude(&["*"]);

    test.send_scheduler(SchedulerMessage::Startup).await;
    test.send_scheduler(SchedulerMessage::PauseAllProcessing)
        .await;
    test.wait_for_message(MessageFromScheduler::IndexingFinished(AssetRootDirId(1)))
        .await;

    {
        let sections_resp: TimelineSectionsResponse =
            test.app.get("/api/timeline/sections").await.json().await;
        assert_eq!(sections_resp.sections.len(), 0);
        assert_eq!(sections_resp.months_summary.len(), 0);
    }

    test.add_includes(&[
        "GP010797.JPG",
        "GP010822.JPG",
        "GP010824.JPG",
        "20250625_*.jpg",
        "20250626_*.jpg",
        "20251231_170735.jpg",
    ]);
    test.reindex_wait().await;
    assert_eq!(
        test.db_query(repository::asset::get_assets)
            .await
            .unwrap()
            .len(),
        7
    );

    {
        let sections_resp: TimelineSectionsResponse =
            test.app.get("/api/timeline/sections").await.json().await;
        assert_eq!(sections_resp.sections.len(), 1);
        assert_eq!(sections_resp.months_summary.len(), 1);
        assert_eq!(sections_resp.months_summary[0].len(), 3);
        assert_matches!(
            &sections_resp.months_summary[0][0],
            TimelineMonthSlice {
                year: 2026,
                month: 5,
                num_assets: 3,
                total_normalized_width: _
            }
        );
        assert_matches!(
            &sections_resp.months_summary[0][1],
            TimelineMonthSlice {
                year: 2025,
                month: 12,
                num_assets: 1,
                total_normalized_width: _
            }
        );
        assert_matches!(
            &sections_resp.months_summary[0][2],
            TimelineMonthSlice {
                year: 2025,
                month: 6,
                num_assets: 3,
                total_normalized_width: _
            }
        );
    }
    {
        let segments: TimelineSegmentsResponse =
            test.app.get("/api/timeline/sections/1").await.json().await;
        assert_eq!(segments.segments.len(), 5);
        let want_assets: &[&[&str]] = &[
            &["GP010824.JPG", "GP010822.JPG"],
            &["GP010797.JPG"],
            &["20251231_170735.jpg"],
            &["20250626_223936.jpg", "20250626_175438.jpg"],
            &["20250625_154822.jpg"],
        ];
        for (segment, want) in segments.segments.iter().zip_eq(want_assets) {
            for (item, want_filename) in segment.items.iter().zip_eq(*want) {
                match item {
                    TimelineItem::Asset(asset_with_spe) => {
                        assert_eq!(
                            asset_with_spe.asset.rep_file.path_in_root, *want_filename,
                            "{:?}",
                            segments.segments
                        );
                    }
                    other => panic!("expected Asset, got {other:?}"),
                }
            }
        }
    }
    let asset1 = test.asset_with_path("GP010822.JPG").await;
    let asset2 = test.asset_with_path("20251231_170735.jpg").await;
    let asset4 = test.asset_with_path("20250625_154822.jpg").await;
    let asset1_id = asset1.base.id;

    {
        let group_resp: CreateTimelineGroupResponse = test
            .app
            .post(
                "/api/timelinegroups",
                &CreateTimelineGroupRequest {
                    assets: vec![asset4.base.id.into()],
                    name: "unrelated group".to_owned(),
                },
            )
            .await
            .json()
            .await;
        assert_eq!(group_resp.display_date, asset4.base.taken_date);
    }

    // add asset1 and asset2
    let group = {
        let group_resp: CreateTimelineGroupResponse = test
            .app
            .post(
                "/api/timelinegroups",
                &CreateTimelineGroupRequest {
                    assets: vec![asset1.base.id.into(), asset2.base.id.into()],
                    name: "test group".to_owned(),
                },
            )
            .await
            .json()
            .await;
        assert_eq!(group_resp.display_date, asset1.base.taken_date);

        let segments: TimelineSegmentsResponse =
            test.app.get("/api/timeline/sections/1").await.json().await;
        assert_eq!(segments.segments.len(), 5);
        let want_assets: &[&[&str]] = &[
            &["GP010824.JPG"],
            &["GP010822.JPG", "20251231_170735.jpg"],
            &["GP010797.JPG"],
            &["20250626_223936.jpg", "20250626_175438.jpg"],
            &["20250625_154822.jpg"],
        ];
        for (segment, want) in segments.segments.iter().zip_eq(want_assets) {
            for (item, want_filename) in segment.items.iter().zip_eq(*want) {
                match item {
                    TimelineItem::Asset(asset_with_spe) => {
                        assert_eq!(
                            asset_with_spe.asset.rep_file.path_in_root, *want_filename,
                            "{:?}",
                            segments.segments
                        );
                    }
                    other => panic!("expected Asset, got {other:?}"),
                }
            }
        }
        assert_eq!(
            segments.segments[1].segment,
            SegmentType::UserGroup {
                id: group_resp.timeline_group_id.clone(),
                name: Some("test group".to_owned())
            }
        );
        let group_id: model::TimelineGroupId = group_resp.timeline_group_id.try_into().unwrap();
        let assets_in_group = test
            .db_query(move |conn| repository::timeline_group::get_assets_in_group(conn, group_id))
            .await
            .unwrap();
        assert_eq!(assets_in_group, &[asset1.base.clone(), asset2.base.clone()]);
        let group_for_asset = test
            .db_query(move |conn| {
                repository::timeline_group::get_timeline_group_for_asset(conn, asset1_id)
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(group_for_asset.id, group_id);
        assert_eq!(group_for_asset.name.as_deref(), Some("test group"));
        group_for_asset
    };

    let str_group_id = TimelineGroupId::from(group.id);
    let asset3 = test.asset_with_path("20250626_175438.jpg").await;
    // add asset 3
    {
        assert!(
            !test
                .app
                .patch(
                    &format!("/api/timelinegroups/{str_group_id}"),
                    &EditTimelineGroupRequest {
                        assets: vec![asset3.base.id.into()],
                        operation: EditTimelineGroup::Remove,
                    },
                )
                .await
                .status()
                .is_success(),
            "removing Asset that's not in group should return an error"
        );
        assert_eq!(
            test.app
                .patch(
                    &format!("/api/timelinegroups/{str_group_id}"),
                    &EditTimelineGroupRequest {
                        assets: vec![asset3.base.id.into()],
                        operation: EditTimelineGroup::Add,
                    },
                )
                .await
                .status(),
            StatusCode::OK
        );

        let want_assets: &[&[&str]] = &[
            &["GP010824.JPG"],
            &["GP010822.JPG", "20251231_170735.jpg", "20250626_175438.jpg"],
            &["GP010797.JPG"],
            &["20250626_223936.jpg"],
            &["20250625_154822.jpg"],
        ];
        let segments: TimelineSegmentsResponse =
            test.app.get("/api/timeline/sections/1").await.json().await;
        for (segment, want) in segments.segments.iter().zip_eq(want_assets) {
            for (item, want_filename) in segment.items.iter().zip_eq(*want) {
                match item {
                    TimelineItem::Asset(asset_with_spe) => {
                        assert_eq!(
                            asset_with_spe.asset.rep_file.path_in_root, *want_filename,
                            "{:?}",
                            segments.segments
                        );
                    }
                    other => panic!("expected Asset, got {other:?}"),
                }
            }
        }
        assert_eq!(
            segments.segments[1].segment,
            SegmentType::UserGroup {
                id: group.id.into(),
                name: Some("test group".to_owned())
            }
        );
    }
    // remove asset1
    {
        let remove_resp = test
            .app
            .patch(
                &format!("/api/timelinegroups/{str_group_id}"),
                &EditTimelineGroupRequest {
                    assets: vec![asset1.base.id.into()],
                    operation: EditTimelineGroup::Remove,
                },
            )
            .await;
        assert_eq!(remove_resp.status(), StatusCode::OK,);

        let add_again_resp = test
            .app
            .patch(
                &format!("/api/timelinegroups/{str_group_id}"),
                &EditTimelineGroupRequest {
                    assets: vec![asset1.base.id.into()],
                    operation: EditTimelineGroup::Remove,
                },
            )
            .await;
        assert!(!add_again_resp.status().is_success());

        let want_assets: &[&[&str]] = &[
            &["GP010824.JPG", "GP010822.JPG"],
            &["GP010797.JPG"],
            &["20251231_170735.jpg", "20250626_175438.jpg"],
            &["20250626_223936.jpg"],
            &["20250625_154822.jpg"],
        ];
        let segments: TimelineSegmentsResponse =
            test.app.get("/api/timeline/sections/1").await.json().await;
        for (segment, want) in segments.segments.iter().zip_eq(want_assets) {
            for (item, want_filename) in segment.items.iter().zip_eq(*want) {
                match item {
                    TimelineItem::Asset(asset_with_spe) => {
                        assert_eq!(
                            asset_with_spe.asset.rep_file.path_in_root, *want_filename,
                            "{:?}",
                            segments.segments
                        );
                    }
                    other => panic!("expected Asset, got {other:?}"),
                }
            }
        }
        assert_eq!(
            segments.segments[2].segment,
            SegmentType::UserGroup {
                id: group.id.into(),
                name: Some("test group".to_owned())
            }
        );
        assert_eq!(segments.segments[2].sort_date, asset2.base.taken_date);

        let no_group_for_asset = test
            .db_query(move |conn| {
                repository::timeline_group::get_timeline_group_for_asset(conn, asset1_id)
            })
            .await
            .unwrap();
        assert_matches!(no_group_for_asset, None);

        let group_for_asset = test
            .db_query(move |conn| {
                repository::timeline_group::get_timeline_group_for_asset(conn, asset3.base.id)
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(group_for_asset.id, group.id);
    }
    {
        assert_eq!(
            test.app
                .patch(
                    &format!("/api/timelinegroups/{str_group_id}"),
                    &EditTimelineGroupRequest {
                        assets: vec![asset2.base.id.into(), asset3.base.id.into()],
                        operation: EditTimelineGroup::Remove,
                    },
                )
                .await
                .status(),
            StatusCode::OK
        );

        let deleted_group = test
            .db_query(move |conn| repository::timeline_group::get_timeline_group(conn, group.id))
            .await;
        _ = assert_err!(deleted_group);

        let unrelated_group = test
            .db_query(move |conn| {
                repository::timeline_group::get_timeline_group_for_asset(conn, asset4.base.id)
            })
            .await
            .unwrap()
            .expect("unrelated group should not have been deleted");
        assert_some_eq!(unrelated_group.name, "unrelated group".to_owned());
    }
}

#[tokio::test]
async fn timeline_group_and_series() {
    setup_tracing();

    let mut test = Test::new().await;

    test.set_exclude(&["*"]);

    test.send_scheduler(SchedulerMessage::Startup).await;
    test.send_scheduler(SchedulerMessage::PauseAllProcessing)
        .await;
    test.wait_for_message(MessageFromScheduler::IndexingFinished(AssetRootDirId(1)))
        .await;

    {
        let sections_resp: TimelineSectionsResponse =
            test.app.get("/api/timeline/sections").await.json().await;
        assert_eq!(sections_resp.sections.len(), 0);
        assert_eq!(sections_resp.months_summary.len(), 0);
    }

    test.add_includes(&[
        "timelapse/raw-and-jpeg/P107024?.*", // ignore _copy.JPG for now
        "timelapse/jpeg-only-1/*",
        "timelapse/jpeg-only-2-start/*",
    ]);
    test.reindex_wait().await;

    {
        let sections_resp: TimelineSectionsResponse =
            test.app.get("/api/timeline/sections").await.json().await;
        assert_eq!(sections_resp.sections.len(), 1);
        assert_eq!(sections_resp.months_summary.len(), 1);
        assert_eq!(sections_resp.months_summary[0].len(), 1);
        assert_matches!(
            &sections_resp.months_summary[0][0],
            TimelineMonthSlice {
                year: 2016,
                month: 8,
                num_assets: 3, // 3 merged series, not 3 assets
                total_normalized_width: _
            }
        );
    }
    {
        let segments: TimelineSegmentsResponse =
            test.app.get("/api/timeline/sections/1").await.json().await;
        assert_eq!(segments.segments.len(), 2);
        assert_eq!(segments.segments[0].items.len(), 2);

        let seg1 = &segments.segments[1];
        assert_eq!(
            seg1.segment,
            SegmentType::DateRange {
                // AssetSeries gets assigned a single date, so start and end are equal
                start: DateTime::parse_from_rfc3339("2016-08-08T11:51:28.433+02:00")
                    .unwrap()
                    .to_utc(),
                end: DateTime::parse_from_rfc3339("2016-08-08T11:51:28.433+02:00")
                    .unwrap()
                    .to_utc(),
            }
        );

        match seg1.items.first().unwrap() {
            TimelineItem::AssetSeries {
                series_id: _,
                assets,
                selection_indices,
            } => {
                assert_eq!(
                    assets.first().unwrap().asset.taken_date,
                    DateTime::parse_from_rfc3339("2016-08-08T11:51:28.433+02:00").unwrap()
                );
                assert_eq!(
                    assets.last().unwrap().asset.taken_date,
                    DateTime::parse_from_rfc3339("2016-08-08T11:51:03.291+02:00").unwrap()
                );
                assert_eq!(selection_indices, &[assets.len() - 1]);
            }
            other => panic!("expected AssetSeries, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn timelapse_raw_jpeg() {
    setup_tracing();

    let mut test = Test::new().await;

    test.set_exclude(&["*"]);

    test.send_scheduler(SchedulerMessage::Startup).await;
    test.send_scheduler(SchedulerMessage::PauseAllProcessing)
        .await;
    test.wait_for_message(MessageFromScheduler::IndexingFinished(AssetRootDirId(1)))
        .await;

    {
        let sections_resp: TimelineSectionsResponse =
            test.app.get("/api/timeline/sections").await.json().await;
        assert_eq!(sections_resp.sections.len(), 0);
        assert_eq!(sections_resp.months_summary.len(), 0);
    }

    test.add_includes(&["timelapse/raw-and-jpeg/P107024{4,5,6}.RW2"]);
    test.reindex_wait().await;

    {
        let sections_resp: TimelineSectionsResponse =
            test.app.get("/api/timeline/sections").await.json().await;
        assert_eq!(sections_resp.sections.len(), 1, "{sections_resp:?}");
        assert_eq!(sections_resp.months_summary.len(), 1);
        assert_eq!(sections_resp.months_summary[0].len(), 1);
    }

    {
        let conn = test.db_pool.get().await.unwrap();
        let all_files = interact!(conn, move |conn| {
            repository::asset::get_all_asset_files(conn)
        })
        .await
        .unwrap()
        .unwrap();
        let all_assets = interact!(conn, move |conn| { repository::asset::get_assets(conn) })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(all_files.len(), 3);
        assert_eq!(all_files.len(), all_assets.len());
        let all_series = interact!(conn, move |conn| {
            repository::asset_series::get_all_series(conn)
        })
        .await
        .unwrap()
        .unwrap();
        assert_eq!(all_series.len(), 1);
    }
    {
        let all_assets = test.db_query(repository::asset::get_assets).await.unwrap();
        let first_asset_id = all_assets[0].base.id;
        let series = test
            .db_query(move |conn| {
                repository::asset_series::get_series_for_asset(conn, first_asset_id)
            })
            .await
            .unwrap()
            .unwrap();
        let TimelineSegmentsResponse { segments }: TimelineSegmentsResponse =
            test.app.get("/api/timeline/sections/1").await.json().await;
        assert_eq!(segments.len(), 1, "{segments:?}");
        assert_eq!(segments[0].items.len(), 1, "{segments:?}");
        match &segments[0].items[0] {
            TimelineItem::AssetSeries {
                series_id,
                assets,
                selection_indices,
            } => {
                assert_eq!(*series_id, AssetSeriesId::from(series.series_id));
                let expected_names = [
                    "timelapse/raw-and-jpeg/P1070246.RW2",
                    "timelapse/raw-and-jpeg/P1070245.RW2",
                    "timelapse/raw-and-jpeg/P1070244.RW2",
                ];
                let actual_names = assets
                    .iter()
                    .map(|a| a.asset.rep_file.path_in_root.as_str())
                    .collect_vec();
                assert_eq!(expected_names.as_slice(), &actual_names);
                assert_eq!(1, selection_indices.len());
            }
            other => panic!("expected AssetSeries item, got {other:?}"),
        }
    }

    test.add_includes(&["timelapse/raw-and-jpeg/P107024{4,5,6}.JPG"]);
    test.reindex_wait().await;

    {
        let all_files = test
            .db_query(repository::asset::get_all_asset_files)
            .await
            .unwrap();
        let all_assets = test.db_query(repository::asset::get_assets).await.unwrap();
        assert_eq!(all_files.len(), 6);
        assert_eq!(all_assets.len(), 3);

        let all_series = test
            .db_query(repository::asset_series::get_all_series)
            .await
            .unwrap();
        assert_eq!(
            all_series.len(),
            1,
            "{:?}",
            all_assets
                .iter()
                .map(|asset| (&asset.rep_file.file_path, &asset.base.in_series))
                .collect_vec()
        );

        let sections_resp: TimelineSectionsResponse = test
            .app
            .get(&format!(
                "/api/timeline/sections?initialAssetId={}",
                all_assets[0].base.id.0
            ))
            .await
            .json()
            .await;
        assert_eq!(sections_resp.sections.len(), 1);
        assert_matches!(sections_resp.initial_asset_section,
            Some(TimelineSegmentsWithId {
                section_id: _,
                segments,
            }) if segments.len() == 1
        );

        let first_asset_id = all_assets[0].base.id;
        let series = test
            .db_query(move |conn| {
                repository::asset_series::get_series_for_asset(conn, first_asset_id)
            })
            .await
            .unwrap()
            .unwrap();
        let TimelineSegmentsResponse { segments }: TimelineSegmentsResponse =
            test.app.get("/api/timeline/sections/1").await.json().await;
        assert_eq!(segments.len(), 1, "{segments:?}");
        assert_eq!(segments[0].items.len(), 1, "{segments:?}");
        match &segments[0].items[0] {
            TimelineItem::AssetSeries {
                series_id,
                assets,
                selection_indices,
            } => {
                assert_eq!(*series_id, AssetSeriesId::from(series.series_id));
                let expected_names = [
                    "timelapse/raw-and-jpeg/P1070246.JPG",
                    "timelapse/raw-and-jpeg/P1070245.JPG",
                    "timelapse/raw-and-jpeg/P1070244.JPG",
                ];
                let actual_names = assets
                    .iter()
                    .map(|a| a.asset.rep_file.path_in_root.as_str())
                    .collect_vec();
                assert_eq!(expected_names.as_slice(), &actual_names);
                assert_eq!(selection_indices.len(), 1);
            }
            other => panic!("expected AssetSeries item, got {other:?}"),
        }

        let asset_46 = test
            .asset_with_path("timelapse/raw-and-jpeg/P1070246.JPG")
            .await;
        let asset_45 = test
            .asset_with_path("timelapse/raw-and-jpeg/P1070245.JPG")
            .await;
        let asset_44 = test
            .asset_with_path("timelapse/raw-and-jpeg/P1070244.JPG")
            .await;
        let resp: SetAssetIsSeriesSelectionResponse = test
            .app
            .post(
                format!("/api/assets/{}/seriesSelection", asset_46.base.id.0),
                &SetAssetSeriesSelectionRequest {
                    is_series_selection: true,
                },
            )
            .await
            .json()
            .await;
        assert_eq!(
            resp.asset_ids[resp.selection_indices[1]],
            AssetId::from(asset_44.base.id)
        );
        assert_eq!(
            resp.asset_ids[resp.selection_indices[0]],
            AssetId::from(asset_46.base.id)
        );
        assert_eq!(&resp.selection_indices, &[0, 2]);

        let segments: TimelineSegmentsResponse =
            test.app.get("/api/timeline/sections/1").await.json().await;
        assert_eq!(segments.segments.len(), 1);
        assert_eq!(segments.segments[0].items.len(), 1);
        match &segments.segments[0].items[0] {
            TimelineItem::AssetSeries {
                series_id,
                assets,
                selection_indices,
            } => {
                assert_eq!(*series_id, series.series_id.into());
                assert_eq!(selection_indices, &[0, 2]);
                let series_asset_ids = assets
                    .iter()
                    .map(|asset| model::AssetId::try_from(&asset.asset.asset_id).unwrap())
                    .collect_vec();
                assert_eq!(
                    &series_asset_ids,
                    &[asset_46.base.id, asset_45.base.id, asset_44.base.id]
                );
            }
            other => panic!("expected AssetSeries timeline item, got {other:?}"),
        }
        assert_matches!(
            &segments.segments[0].items[0],
            TimelineItem::AssetSeries {
                series_id,
                assets,
                selection_indices
            } if *series_id == series.series_id.into() && selection_indices == &[0, 2]
        );

        let resp: SetAssetIsSeriesSelectionResponse = test
            .app
            .post(
                format!("/api/assets/{}/seriesSelection", asset_44.base.id.0),
                &SetAssetSeriesSelectionRequest {
                    is_series_selection: false,
                },
            )
            .await
            .json()
            .await;
        assert_eq!(
            resp.asset_ids[resp.selection_indices[0]],
            AssetId::from(asset_46.base.id)
        );
        assert_eq!(&resp.selection_indices, &[0]);

        let segments: TimelineSegmentsResponse =
            test.app.get("/api/timeline/sections/1").await.json().await;
        assert_eq!(segments.segments.len(), 1);
        assert_eq!(segments.segments[0].items.len(), 1);
        assert_matches!(
            &segments.segments[0].items[0],
            TimelineItem::AssetSeries {
                series_id,
                assets,
                selection_indices
            } if *series_id == series.series_id.into() && selection_indices == &[0]
        );
    }

    test.add_includes(&[
        "timelapse/raw-and-jpeg/P107024?.*", // ignore _copy.JPG for now
        "20220423_085935.heif",
        "GP010822.JPG",
    ]);
    test.reindex_wait().await;

    {
        let asset_46 = test
            .asset_with_path("timelapse/raw-and-jpeg/P1070246.JPG")
            .await;
        let asset_49 = test
            .asset_with_path("timelapse/raw-and-jpeg/P1070249.JPG")
            .await;
        let series = test
            .db_query(move |conn| {
                repository::asset_series::get_series_for_asset(conn, asset_49.base.id)
            })
            .await
            .unwrap()
            .unwrap();
        let segments: TimelineSegmentsResponse =
            test.app.get("/api/timeline/sections/1").await.json().await;
        assert_eq!(segments.segments.len(), 3);
        assert_eq!(segments.segments[0].items.len(), 1);
        assert_eq!(segments.segments[1].items.len(), 1);
        assert_eq!(segments.segments[2].items.len(), 1);
        assert_matches!(
            &segments.segments[0].items[0],
            TimelineItem::Asset(AssetWithSpe { asset, spe: AssetSpe::Image(_) }) if asset.rep_file.path_in_root == "GP010822.JPG"
        );
        assert_matches!(
            &segments.segments[1].items[0],
            TimelineItem::Asset(AssetWithSpe { asset, spe: AssetSpe::Image(_) }) if asset.rep_file.path_in_root == "20220423_085935.heif"
        );
        assert_matches!(
            &segments.segments[2].items[0],
            TimelineItem::AssetSeries {
                series_id,
                assets,
                selection_indices
            } if *series_id == series.series_id.into() && selection_indices == &[3]
                && assets[3].asset.asset_id == asset_46.base.id.into()
        );
    }

    test.scheduler
        .send
        .send(SchedulerMessage::Shutdown)
        .await
        .unwrap();
    drop(test.scheduler);
    drop(test.app);

    loop {
        let received = test.scheduler_recv.recv().await;
        match received {
            Err(RecvError::Closed) => break,
            Err(err) => {
                tracing::error!(?err);
            }
            Ok(msg) => {
                tracing::info!(?msg);
            }
        }
    }
}

#[tokio::test]
async fn moving_asset_files_works() {
    setup_tracing();

    let mut test = Test::new().await;

    test.set_exclude(&["*"]);

    test.send_scheduler(SchedulerMessage::Startup).await;
    test.send_scheduler(SchedulerMessage::PauseAllProcessing)
        .await;
    test.wait_for_message(MessageFromScheduler::IndexingFinished(AssetRootDirId(1)))
        .await;

    test.add_includes(&["GP010824*"]);

    test.reindex_wait().await;

    {
        let assets = test.db_query(repository::asset::get_assets).await.unwrap();
        let asset_paths = assets
            .iter()
            .map(|a| a.rep_file.file_path.clone())
            .collect_vec();
        assert_eq!(asset_paths, &["GP010824.JPG"]);

        let resp = test
            .app
            .get(&format!("/api/files/{}/original", assets[0].rep_file.id))
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get(header::CONTENT_TYPE)
                .map(|h| h.to_str().unwrap()),
            Some("image/jpeg")
        );
    }

    tokio::fs::rename(
        test.asset_dir.child("GP010824.JPG"),
        test.asset_dir.child("GP010824.jpg"),
    )
    .await
    .unwrap();

    test.reindex_wait().await;

    {
        let assets = test.db_query(repository::asset::get_assets).await.unwrap();
        let asset_paths = assets
            .iter()
            .map(|a| a.rep_file.file_path.clone())
            .collect_vec();
        assert_eq!(asset_paths, &["GP010824.JPG"]);

        let resp = test
            .app
            .get(&format!("/api/files/{}/original", assets[0].rep_file.id))
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get(header::CONTENT_TYPE)
                .map(|h| h.to_str().unwrap()),
            Some("image/jpeg")
        );
    }
}

pub trait RouterTestExt {
    async fn get(&mut self, url: impl AsRef<str>) -> Response<Body>;
    async fn post<T>(&mut self, url: impl AsRef<str>, json: &T) -> Response<Body>
    where
        T: Serialize;
    async fn patch<T>(&mut self, url: impl AsRef<str>, json: &T) -> Response<Body>
    where
        T: Serialize;
}

pub trait ResponseTestExt {
    async fn json<T>(self) -> T
    where
        T: DeserializeOwned;
}

impl RouterTestExt for Router<()> {
    async fn get(&mut self, url: impl AsRef<str>) -> Response<Body> {
        let req = axum::http::Request::get(url.as_ref())
            .body(Body::empty())
            .unwrap();

        ServiceExt::<axum::http::Request<Body>>::ready(self)
            .await
            .unwrap()
            .call(req)
            .await
            .unwrap()
    }

    async fn post<T>(&mut self, url: impl AsRef<str>, json: &T) -> Response<Body>
    where
        T: Serialize,
    {
        let req = axum::http::Request::post(url.as_ref())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(json).unwrap()))
            .unwrap();

        ServiceExt::<axum::http::Request<Body>>::ready(self)
            .await
            .unwrap()
            .call(req)
            .await
            .unwrap()
    }

    async fn patch<T>(&mut self, url: impl AsRef<str>, json: &T) -> Response<Body>
    where
        T: Serialize,
    {
        let req = axum::http::Request::patch(url.as_ref())
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(json).unwrap()))
            .unwrap();

        ServiceExt::<axum::http::Request<Body>>::ready(self)
            .await
            .unwrap()
            .call(req)
            .await
            .unwrap()
    }
}

impl ResponseTestExt for Response<Body> {
    async fn json<T>(self) -> T
    where
        T: DeserializeOwned,
    {
        assert_eq!(self.status(), StatusCode::OK);
        let b = self.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&b).unwrap()
    }
}
