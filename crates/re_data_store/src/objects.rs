use re_log_types::{IndexHash, MsgId, ObjPath};

/// Common properties of an object instance.
#[derive(Copy, Clone, Debug)]
pub struct InstanceProps<'s> {
    // NOTE: While we would normally make InstanceProps generic over time
    // (`InstanceProps<'s, Time`>), doing so leads to a gigantic template-leak that
    // propagates all over the codebase.
    // So for now we will constrain ourselves to an i64 here, which is the only unit
    // of time we currently use in practice anyway.
    pub time: i64,
    pub msg_id: &'s MsgId,
    pub color: Option<[u8; 4]>,

    /// Use this to test if the object should be visible, etc.
    pub obj_path: &'s ObjPath,

    /// If it is a multi-object, this is the instance index,
    /// else it is [`IndexHash::NONE`].
    pub instance_index: IndexHash,

    /// Whether or not the object is visible
    pub visible: bool,
}
