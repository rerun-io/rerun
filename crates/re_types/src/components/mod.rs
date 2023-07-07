// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.

mod class_id;
mod class_id_ext;
mod color;
mod color_ext;
mod draw_order;
mod draw_order_ext;
mod fuzzy;
mod instance_key;
mod instance_key_ext;
mod keypoint_id;
mod keypoint_id_ext;
mod label;
mod label_ext;
mod point2d;
mod point2d_ext;
mod radius;
mod radius_ext;

pub use self::class_id::ClassId;
pub use self::color::Color;
pub use self::draw_order::DrawOrder;
pub use self::fuzzy::{
    AffixFuzzer1, AffixFuzzer10, AffixFuzzer11, AffixFuzzer12, AffixFuzzer13, AffixFuzzer14,
    AffixFuzzer16, AffixFuzzer17, AffixFuzzer18, AffixFuzzer2, AffixFuzzer3, AffixFuzzer4,
    AffixFuzzer5, AffixFuzzer6, AffixFuzzer7, AffixFuzzer8, AffixFuzzer9,
};
pub use self::instance_key::InstanceKey;
pub use self::keypoint_id::KeypointId;
pub use self::label::Label;
pub use self::point2d::Point2D;
pub use self::radius::Radius;
