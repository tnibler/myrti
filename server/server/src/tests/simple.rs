use std::assert_matches;
use std::sync::{Arc, Mutex};

use axum::http::header;
use axum::{
    Router,
    body::Body,
    http::{Request, Response, StatusCode},
};
use camino::{Utf8Path as Path, Utf8PathBuf as PathBuf};
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
use serde_json::json;
use tokio::sync::broadcast::{self, error::RecvError};
use tower::{Service, ServiceExt};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::routes::asset::{SetAssetIsSeriesSelectionResponse, SetAssetSeriesSelectionRequest};
use crate::schema::AssetId;
use crate::schema::asset::{AssetSpe, AssetWithSpe};
use crate::{
    routes::{
        photo_series::AddAssetsToSeriesRequest,
        timeline::{
            TimelineItem, TimelineSectionsResponse, TimelineSegmentsResponse,
            TimelineSegmentsWithId,
        },
    },
    schema::{AssetSeriesId, asset_series::AssetSeries},
    server::{Server, SetupConfig, make_app},
};

pub struct Test {
    pub app: Router<()>,
    pub scheduler: SchedulerHandle,
    pub scheduler_recv: broadcast::Receiver<MessageFromScheduler>,
    pub db_pool: DbPool,
    pub config: Arc<Mutex<Config>>,
}

impl Test {
    pub async fn new() -> Self {
        let config = Config {
            asset_dirs: vec![AssetDir {
                name: None,
                path: PathBuf::from("../test-library"),
                exclude_globs: Default::default(),
                include_globs: Default::default(),
            }],
            data_dir: DataDir {
                path: PathBuf::from("data_directory"),
                name: None,
                db_path: None,
            },
            bin_paths: Some(BinPaths {
                exiftool: Some(PathBuf::from("../test-shims/exiftool.sh")),
                ffmpeg: Some(PathBuf::from("../test-shims/ffmpeg.sh")),
                ffprobe: Some(PathBuf::from("../test-shims/ffprobe.sh")),
                gpac: Some(PathBuf::from("../test-shims/gpac.sh")),
            }),
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

#[tokio::test()]
async fn simple_integration() {
    let tracing = tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_env("MYRTI_LOG"))
        .with(tracing_error::ErrorLayer::default())
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_target(true)
                .with_file(false)
                .with_line_number(false)
                .with_writer(std::io::stderr),
        );
    tracing.init();

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
            } if *series_id == series.series_id.into() && selection_indices == &[2]
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

pub trait RouterTestExt {
    async fn get(&mut self, url: impl AsRef<str>) -> Response<Body>;
    async fn post<T>(&mut self, url: impl AsRef<str>, json: &T) -> Response<Body>
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
