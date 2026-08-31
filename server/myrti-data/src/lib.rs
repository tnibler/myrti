pub use deadpool_diesel;
pub mod db;
pub mod model;
pub mod repository;

#[macro_export]
macro_rules! interact {
    ($conn:ident, $block:expr) => {
        tracing::Instrument::in_current_span(<_ as futures::TryFutureExt>::map_err(
            $conn.interact::<_, eyre::Result<_>>($block),
            |err| match err {
                ::myrti_data::deadpool_diesel::InteractError::Panic(_) => {
                    eyre::eyre!("database interaction panicked")
                }
                ::myrti_data::deadpool_diesel::InteractError::Aborted => {
                    eyre::eyre!("database interaction was aborted")
                }
            },
        ))
    };
}
