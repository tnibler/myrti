use myrti_core::{
    deadpool_diesel, interact,
    model::{
        self,
        repository::{self, db::PooledDbConn},
    },
};

use crate::schema::asset::{AssetSpe, AssetWithSpe, Image, ImageRepresentation, Video};

#[tracing::instrument(level = "trace", skip(conn))]
pub async fn get_full_asset(
    conn: &mut PooledDbConn,
    asset: model::Asset,
) -> eyre::Result<AssetWithSpe> {
    match &asset.sp {
        model::AssetSpe::Image(image) => {
            let file_id = image.file_id;
            let reprs = interact!(conn, move |conn| {
                repository::representation::get_image_representations(conn, file_id)
            })
            .await??;
            let api_reprs = reprs
                .into_iter()
                .map(|repr| ImageRepresentation {
                    id: repr.id.0.to_string(),
                    format: repr.format_name,
                    width: repr.width,
                    height: repr.height,
                    size: repr.file_size,
                })
                .collect();
            Ok(AssetWithSpe {
                asset: asset.into(),
                spe: AssetSpe::Image(Image {
                    representations: api_reprs,
                }),
            })
        }
        model::AssetSpe::Video(_video) => Ok(AssetWithSpe {
            asset: asset.into(),
            spe: AssetSpe::Video(Video {
                has_dash: true, // FIXME: field doesn't exist anymore
            }),
        }),
    }
}
