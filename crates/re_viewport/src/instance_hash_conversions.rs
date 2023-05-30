use re_data_store::InstancePathHash;
use re_log_types::{component_types::InstanceKey, EntityPathHash};
use re_renderer::{PickingLayerId, PickingLayerInstanceId, PickingLayerObjectId};

#[inline]
pub fn picking_layer_id_from_instance_path_hash(value: InstancePathHash) -> PickingLayerId {
    PickingLayerId {
        object: PickingLayerObjectId(value.entity_path_hash.hash64()),
        instance: PickingLayerInstanceId(value.instance_key.0),
    }
}

#[inline]
pub fn instance_path_hash_from_picking_layer_id(value: PickingLayerId) -> InstancePathHash {
    InstancePathHash {
        entity_path_hash: EntityPathHash::from_u64(value.object.0),
        instance_key: InstanceKey(value.instance.0),
    }
}
