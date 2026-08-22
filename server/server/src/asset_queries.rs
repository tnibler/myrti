use myrti_core::model::{self, repository};

use crate::schema::asset::{AssetSpe, AssetWithSpe, Image, Video};

pub fn make_api_asset(
    asset: model::Asset,
    extra: repository::timeline::AssetInTimelineExtra,
) -> AssetWithSpe {
    match (&asset.sp, extra) {
        (
            model::AssetSpe::Image(_image),
            repository::timeline::AssetInTimelineExtra::Image { representations },
        ) => AssetWithSpe {
            asset: asset.into(),
            spe: AssetSpe::Image(Image {
                representations: serde_json::value::RawValue::from_string(representations)
                    .expect("sqlite produces valid json"),
            }),
        },
        (model::AssetSpe::Video(_video), repository::timeline::AssetInTimelineExtra::Video {}) => {
            AssetWithSpe {
                asset: asset.into(),
                spe: AssetSpe::Video(Video {
                    has_dash: true, // FIXME: field doesn't exist anymore
                }),
            }
        }
        other => {
            panic!("mismatched AssetSpe and AssetInTimelineExtra: {:?}", other)
        }
    }
}
