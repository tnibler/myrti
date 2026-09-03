use camino::Utf8PathBuf as PathBuf;

use myrti_core::{
    config::{AssetDir, BinPaths, Config, DataDir},
    core::{scheduler::MessageFromScheduler, storage::Storage},
};
use tokio::sync::broadcast::error::RecvError;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::server::{Server, SetupConfig, make_app};

#[tokio::test()]
async fn simple_integration() {
    let tracing = tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_env("MYRTI_LOG"))
        .with(tracing_error::ErrorLayer::default())
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_target(false)
                .with_file(false)
                .with_line_number(false)
                .with_writer(std::io::stderr),
        );
    tracing.init();
    let config = Config {
        asset_dirs: vec![AssetDir {
            name: None,
            path: PathBuf::from("./test-library"),
            exclude_globs: Default::default(),
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
    let Server {
        app,
        scheduler,
        mut scheduler_recv,
    } = make_app(SetupConfig {
        config,
        config_dir: "config_directory".into(),
        db_url: ":memory:",
        storage,
    })
    .await
    .expect("error creating app");

    // let server = TestServer::builder().expect_success_by_default().build(app);
    // server.get("/api/assets").await.assert_json(&json!([]));

    scheduler
        .send
        .send(myrti_core::core::scheduler::SchedulerMessage::Startup)
        .await
        .unwrap();
    scheduler
        .send
        .send(myrti_core::core::scheduler::SchedulerMessage::PauseAllProcessing)
        .await
        .unwrap();

    match scheduler_recv.recv().await {
        Ok(MessageFromScheduler::IndexingFinished(_)) => {}
        other => panic!("unexpected message received: {:?}", other),
    }

    scheduler
        .send
        .send(myrti_core::core::scheduler::SchedulerMessage::Shutdown)
        .await
        .unwrap();
    drop(scheduler);
    drop(app);

    loop {
        let received = scheduler_recv.recv().await;
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
