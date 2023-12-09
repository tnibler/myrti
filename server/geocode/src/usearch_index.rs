use color_eyre::eyre::{eyre, Context, Result};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{Coordinates, GeonameId};

pub struct UsearchIndex {
    cancel: CancellationToken,
    req_tx: mpsc::Sender<SearchRequest>,
    res_rx: mpsc::Receiver<Result<Vec<SearchResult>>>,
    add_to_index_req_tx: mpsc::Sender<AddToIndex>,
    add_to_index_res_rx: mpsc::Receiver<Result<()>>,
}

#[derive(Debug, Clone)]
pub struct SearchRequest {
    pub val: Coordinates,
    pub n_results: usize,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub geoname_id: GeonameId,
    pub distance: f32,
}

#[derive(Debug, Clone)]
pub struct AddToIndex {
    pub key: u64,
    pub val: Coordinates,
}

impl UsearchIndex {
    pub async fn new() -> Result<UsearchIndex> {
        let cancel = CancellationToken::new();
        let (req_tx, mut req_rx) = mpsc::channel::<SearchRequest>(1000);
        let (res_tx, res_rx) = mpsc::channel::<Result<Vec<SearchResult>>>(1000);
        let (add_req_tx, mut add_req_rx) = mpsc::channel::<AddToIndex>(1000);
        let (add_res_tx, add_res_rx) = mpsc::channel::<Result<()>>(1000);
        let (err_tx, mut err_rx) = mpsc::channel::<color_eyre::Report>(1000);
        let cancel_copy = cancel.clone();

        let rt = tokio::runtime::Builder::new_current_thread().build()?;
        let thread = std::thread::spawn(move || {
            let local_set = tokio::task::LocalSet::default();
            let fut = local_set.run_until(async move {
                    let index_options = usearch::ffi::IndexOptions {
                        multi: false,
                        dimensions: 2,
                        metric: usearch::ffi::MetricKind::Haversine,
                        ..Default::default()
                    };
                    let index =
                    match usearch::new_index(&index_options).wrap_err("error creating usearch index") {
                        Ok(index) => index,
                        Err(e) => {
                            let _ = err_tx.send(e).await;
                            return;
                        }
                    };
                    match index
                        .reserve(100000)
                        .wrap_err("error reserving memory for usearch index")
                        {
                            Err(e) => {
                                let _ = err_tx.send(e).await;
                                return;
                            }
                            Ok(_) => {}
                        };
                    // no errors occured, drop the sender so the receiver knows all is good
                    drop(err_tx);
                    let mutex = tokio::sync::Mutex::<()>::default();
                    loop {
                        tokio::select! {
                            _ = cancel_copy.cancelled() => {
                                break;
                            }
                            Some(search) = req_rx.recv() => {
                                let _guard = mutex.lock().await;
                                let result = index
                                    .search(&[search.val.lat, search.val.lon], search.n_results)
                                    .wrap_err("error searching usearch index");
                                match result {
                                    Err(e) => {
                                        let _ = res_tx.send(Err(e)).await;
                                    },
                                    Ok(result) => {
                                        let results: Vec<_> = result.keys.into_iter().zip(result.distances.into_iter())
                                            .map(|(key, dist)| SearchResult {
                                                geoname_id: GeonameId(key as i64),
                                                distance: dist
                                            })
                                            .collect();
                                        let _ = res_tx.send(Ok(results)).await;
                                    }
                                }
                            }
                            Some(add_to_index) = add_req_rx.recv() => {
                                let _guard = mutex.lock().await;
                                if index.size() == index.capacity() {
                                    let reserve_result = index
                                        .reserve(index.size() + 10000)
                                        .wrap_err("error growing index capacity");
                                    match reserve_result {
                                        Err(e) => {
                                            let _ = add_res_tx.send(Err(e)).await;
                                        }
                                        Ok(_) => {}
                                    };
                                }
                                let add_result = index
                                    .add(
                                        add_to_index.key,
                                        &[add_to_index.val.lat, add_to_index.val.lon],
                                    )
                                    .wrap_err("error adding entry to index");
                                match add_result {
                                    Err(e) => {
                                        let _ = res_tx.send(Err(e)).await;
                                    },
                                    Ok(_) => {}
                                };
                                let _ = add_res_tx.send(Ok(())).await;
                            }
                        }
                    }
            });
            rt.block_on(fut)
        });
        if let Some(err) = err_rx.recv().await {
            Err(err)
        } else {
            Ok(UsearchIndex {
                cancel,
                req_tx,
                res_rx,
                add_to_index_req_tx: add_req_tx,
                add_to_index_res_rx: add_res_rx,
            })
        }
    }

    pub async fn search(&mut self, v: Coordinates, n_results: usize) -> Result<Vec<SearchResult>> {
        self.req_tx
            .send(SearchRequest { val: v, n_results })
            .await?;
        self.res_rx
            .recv()
            .await
            .ok_or(eyre!("usearch index thread died"))?
    }

    pub async fn add_to_index(&mut self, key: u64, val: Coordinates) -> Result<()> {
        self.add_to_index_req_tx
            .send(AddToIndex { key, val })
            .await?;
        self.add_to_index_res_rx
            .recv()
            .await
            .ok_or(eyre!("usearch index thread died"))?
    }

    pub fn close(self) {
        self.cancel.cancel();
    }
}
