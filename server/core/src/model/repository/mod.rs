pub mod album;
pub mod asset;
pub mod asset_root_dir;
pub mod config;
pub mod db;
pub mod db_entity;
pub mod duplicate_asset;
pub mod failed_job;
pub mod representation;
#[allow(non_snake_case)]
mod schema;
#[cfg(test)]
mod test;
pub mod timeline;
pub mod timeline_group;

#[macro_export()]
macro_rules! interact {
    ($conn:ident, $block:expr) => {
        tracing::Instrument::in_current_span(<_ as futures::TryFutureExt>::map_err(
            $conn.interact::<_, eyre::Result<_>>($block),
            |err| match err {
                deadpool_diesel::InteractError::Panic(_) => {
                    eyre::eyre!("database interaction panicked")
                }
                deadpool_diesel::InteractError::Aborted => {
                    eyre::eyre!("database interaction was aborted")
                }
            },
        ))
    };
}
