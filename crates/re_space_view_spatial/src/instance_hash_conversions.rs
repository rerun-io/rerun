use re_entity_db::InstancePathHash;
use re_log_types::{EntityPathHash, Instance};
use re_renderer::{PickingLayerId, PickingLayerInstanceId, PickingLayerObjectId};

#[inline]
pub fn picking_layer_id_from_instance_path_hash(value: InstancePathHash) -> PickingLayerId {
    PickingLayerId {
        object: PickingLayerObjectId(value.entity_path_hash.hash64()),
        instance: PickingLayerInstanceId(value.instance.get()),
    }
}

#[inline]
pub fn instance_path_hash_from_picking_layer_id(value: PickingLayerId) -> InstancePathHash {
    InstancePathHash {
        entity_path_hash: EntityPathHash::from_u64(value.object.0),
        // `PickingLayerId` uses `u64::MAX` to mean "hover and/or select all instances".
        instance: if value.instance.0 == u64::MAX {
            Instance::ALL
        } else {
            Instance::from(value.instance.0)
        },
    }
}
