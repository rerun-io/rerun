use arrow2::array::Array;
use nohash_hasher::IntMap;
use re_log_types::external::arrow2_convert::serialize::TryIntoArrow;
use re_log_types::msg_bundle::MsgBundleError;
use re_log_types::{component_types::InstanceKey, msg_bundle::wrap_in_listarray};

use crate::{
    Component, ComponentBundle, ComponentName, EntityPath, LogMsg, MsgBundle, MsgId,
    SerializableComponent, Session, Time, TimeInt, TimePoint, Timeline, Transform,
};

// ---

#[derive(thiserror::Error, Debug)]
pub enum MsgSenderError {
    #[error(
        "All component collections must have exactly one row (i.e. no batching), got {0:?} instead"
    )]
    MoreThanOneRow(Vec<(ComponentName, usize)>),

    #[error(
        "All component collections must share the same number of instances (i.e. row length) \
            for a given row, got {0:?} instead"
    )]
    MismatchedRowLengths(Vec<(ComponentName, usize)>),

    #[error("Instance keys cannot be splatted")]
    SplattedInstanceKeys,

    #[error("InstanceKey(u64::MAX) is reserved for Rerun internals")]
    IllegalInstanceKey,

    #[error(transparent)]
    PackingError(#[from] MsgBundleError),
}

/// Facilitates building and sending component payloads with the Rerun SDK.
///
/// ```ignore
/// fn log_coordinate_space(
///     session: &mut Session,
///     ent_path: impl Into<EntityPath>,
///     axes: &str,
/// ) -> anyhow::Result<()> {
///     let view_coords: ViewCoordinates = axes
///         .parse()
///         .map_err(|err| anyhow!("couldn't parse {axes:?} as ViewCoordinates: {err}"))?;
///
///     MsgSender::new(ent_path)
///         .with_timeless(true)
///         .with_component(&[view_coords])?
///         .send(session)
///         .map_err(Into::into)
/// }
/// ```
pub struct MsgSender {
    // TODO(cmc): At the moment, a `MsgBundle` can only contain data for a single entity, so
    // this must be known as soon as we spawn the builder.
    // This won't be true anymore once batch insertions land.
    entity_path: EntityPath,

    /// All the different timestamps for this message.
    ///
    /// The logging time is automatically inserted during creation ([`Self::new`]).
    timepoint: TimePoint,
    /// If true, all timestamp data associated with this message will be dropped right before
    /// sending it to Rerun.
    ///
    /// Timeless data is present on all timelines and behaves as if it was recorded infinitely far
    /// into the past.
    timeless: bool,

    /// The expected number of instances for each row of each component collections appended to the
    /// current message.
    ///
    /// Since we don't yet support batch insertions, the number of rows for each component
    /// collection will always be 1.
    /// The number of instances per row, on the other hand, will be decided based upon the first
    /// component collection that's appended.
    num_instances: Option<usize>,
    /// All the instanced component collections that have been appended to this message.
    ///
    /// As of today, they must have exactly 1 row of data (no batching), which itself must have
    /// `Self::num_instances` instance keys.
    instanced: Vec<ComponentBundle>,

    /// All the splatted components that have been appended to this message.
    ///
    /// By definition, all `ComponentBundle`s in this vector will have 1 row (no batching) and more
    /// importantly a single, special instance key for that row.
    splatted: Vec<ComponentBundle>,
}

impl MsgSender {
    /// Starts a new `MsgSender` for the given entity path.
    ///
    /// It is during this call that the logging time for the message is recorded!
    pub fn new(ent_path: impl Into<EntityPath>) -> Self {
        Self {
            entity_path: ent_path.into(),

            timepoint: [(Timeline::log_time(), Time::now().into())].into(),
            timeless: false,

            num_instances: None,
            instanced: Vec::new(),
            splatted: Vec::new(),
        }
    }

    // --- Time ---

    /// Appends a given `timepoint` to the current message.
    ///
    /// This can be called any number of times. In case of collisions, last write wins.
    /// I.e. if `timepoint` contains a timestamp `ts1` for a timeline `my_time` and the current
    /// message already has a timestamp `ts0` for that same timeline, then the new value (`ts1`)
    /// will overwrite the existing value (`ts0`).
    ///
    /// `MsgSender` automatically keeps track of the logging time, which is recorded when
    /// [`Self::new`] is first called.
    #[inline]
    pub fn with_timepoint(mut self, timepoint: TimePoint) -> Self {
        for (timeline, time) in timepoint {
            self.timepoint.insert(timeline, time);
        }
        self
    }

    /// Appends a given `timeline`/`time` pair to the current message.
    ///
    /// This can be called any number of times. In case of collisions, last write wins.
    /// I.e. if the current message already has a timestamp value for that `timeline`, then the
    /// new `time` value that was just passed in will overwrite it.
    ///
    /// `MsgSender` automatically keeps track of the logging time, which is recorded when
    /// [`Self::new`] is first called.
    #[inline]
    pub fn with_time(mut self, timeline: Timeline, time: impl Into<TimeInt>) -> Self {
        self.timepoint.insert(timeline, time.into());
        self
    }

    /// Specifies whether the current message is timeless.
    ///
    /// A timeless message will drop all of its timestamp data before being sent to Rerun.
    /// Timeless data is present on all timelines and behaves as if it was recorded infinitely far
    /// into the past.
    ///
    /// Always `false` by default.
    #[inline]
    pub fn with_timeless(mut self, timeless: bool) -> Self {
        self.timeless = timeless;
        self
    }

    // --- Components ---

    /// Appends a component collection to the current message.
    ///
    /// All component collections stored in the message must have the same row-length (i.e. number
    /// of instances)!
    /// The row-length of the first appended collection is used as ground truth.
    ///
    /// ⚠ This can only be called once per type of component!
    /// The SDK does not yet support batch insertions, which are semantically identical to adding
    /// the same component type multiple times in a single message.
    /// Doing so will return an error when trying to `send()` the message.
    //
    // TODO(#589): batch insertions
    pub fn with_component<'a, C: SerializableComponent>(
        mut self,
        data: impl IntoIterator<Item = &'a C>,
    ) -> Result<Self, MsgSenderError> {
        let bundle = bundle_from_iter(data)?;

        let num_instances = bundle.num_instances(0).unwrap(); // must have exactly 1 row atm

        // If this is the first appended collection, it gets to decide the row-length (i.e. number
        // of instances) of all future collections.
        if self.num_instances.is_none() {
            self.num_instances = Some(num_instances);
        }

        // Detect mismatched row-lengths early on... unless it's a Transform bundle: transforms
        // behave differently and will be sent in their own message!
        if C::name() != Transform::name() && self.num_instances.unwrap() != num_instances {
            let collections = self
                .instanced
                .into_iter()
                .map(|bundle| (bundle.name(), bundle.num_instances(0).unwrap_or(0)))
                .collect();
            return Err(MsgSenderError::MismatchedRowLengths(collections));
        }

        // TODO(cmc): if this is an InstanceKey and it contains u64::MAX, fire IllegalInstanceKey.

        self.instanced.push(bundle);

        Ok(self)
    }

    /// Appends a splatted component to the current message.
    ///
    /// Splatted components apply to all the instance keys of an entity, whatever they may be at
    /// that point in time.
    ///
    /// ⚠ `InstanceKey`s themselves cannot be splatted! Trying to do so will return an error.
    ///
    /// ⚠ This can only be called once per type of component!
    /// The SDK does not yet support batch insertions, which are semantically identical to adding
    /// the same component type multiple times in a single message.
    /// Doing so will return an error when trying to `send()` the message.
    //
    // TODO(#589): batch insertions
    pub fn with_splat<C: SerializableComponent>(mut self, data: C) -> Result<Self, MsgSenderError> {
        if C::name() == InstanceKey::name() {
            return Err(MsgSenderError::SplattedInstanceKeys);
        }

        self.splatted.push(bundle_from_iter(&[data])?);

        Ok(self)
    }

    /// Helper to make it easier to optionally append splatted components.
    ///
    /// See [`Self::with_splat`].
    pub fn with_splat_opt(
        self,
        data: Option<impl SerializableComponent>,
    ) -> Result<Self, MsgSenderError> {
        if let Some(data) = data {
            self.with_splat(data)
        } else {
            Ok(self)
        }
    }

    // --- Send ---

    /// Consumes, packs, sanity checkes and finally sends the message to the currently configured
    /// target of the SDK.
    pub fn send(self, session: &mut Session) -> Result<(), MsgSenderError> {
        let [msg_standard, msg_transforms, msg_splats] = self.into_messages()?;

        if let Some(msg_standard) = msg_standard {
            session.send(LogMsg::ArrowMsg(msg_standard.try_into()?));
        }
        if let Some(msg_transforms) = msg_transforms {
            session.send(LogMsg::ArrowMsg(msg_transforms.try_into()?));
        }
        if let Some(msg_splats) = msg_splats {
            session.send(LogMsg::ArrowMsg(msg_splats.try_into()?));
        }

        Ok(())
    }

    fn into_messages(self) -> Result<[Option<MsgBundle>; 3], MsgSenderError> {
        let Self {
            entity_path,
            timepoint,
            timeless,
            num_instances: _,
            instanced,
            mut splatted,
        } = self;

        if timeless && timepoint.times().len() > 1 {
            re_log::warn_once!("Recorded timepoints in a timeless message, they will be dropped!");
        }

        // clear current timepoint if marked as timeless
        let timepoint = if timeless { [].into() } else { timepoint };

        // separate transforms from the rest
        // TODO(cmc): just use `Vec::drain_filter` once it goes stable...
        let mut all_bundles: Vec<_> = instanced.into_iter().map(Some).collect();
        let standard_bundles: Vec<_> = all_bundles
            .iter_mut()
            .filter(|bundle| bundle.as_ref().unwrap().name() != Transform::name())
            .map(|bundle| bundle.take().unwrap())
            .collect();
        let transform_bundles: Vec<_> = all_bundles
            .iter_mut()
            .filter(|bundle| {
                bundle
                    .as_ref()
                    .map_or(false, |bundle| bundle.name() == Transform::name())
            })
            .map(|bundle| bundle.take().unwrap())
            .collect();
        debug_assert!(all_bundles.into_iter().all(|bundle| bundle.is_none()));

        // TODO(cmc): The sanity checks we do in here can (and probably should) be done in
        // `MsgBundle` instead so that the python SDK benefits from them too... but one step at a
        // time.

        // sanity check: no row-level batching
        let mut rows_per_comptype: IntMap<ComponentName, usize> = IntMap::default();
        for bundle in standard_bundles
            .iter()
            .chain(&transform_bundles)
            .chain(&splatted)
        {
            *rows_per_comptype.entry(bundle.name()).or_default() += bundle.num_rows();
        }
        if rows_per_comptype.values().any(|num_rows| *num_rows > 1) {
            return Err(MsgSenderError::MoreThanOneRow(
                rows_per_comptype.into_iter().collect(),
            ));
        }

        // sanity check: transforms can't handle multiple instances
        let num_transform_instances = transform_bundles
            .get(0)
            .and_then(|bundle| bundle.num_instances(0))
            .unwrap_or(0);
        if num_transform_instances > 1 {
            re_log::warn!("detected Transform component with multiple instances");
        }

        let mut msgs = [(); 3].map(|_| None);

        // Standard
        msgs[0] = (!standard_bundles.is_empty()).then(|| {
            MsgBundle::new(
                MsgId::random(),
                entity_path.clone(),
                timepoint.clone(),
                standard_bundles,
            )
        });

        // Transforms
        msgs[1] = (!transform_bundles.is_empty()).then(|| {
            MsgBundle::new(
                MsgId::random(),
                entity_path.clone(),
                timepoint.clone(),
                transform_bundles,
            )
        });

        // Splats
        msgs[2] = (!splatted.is_empty()).then(|| {
            splatted.push(bundle_from_iter(&[InstanceKey::SPLAT]).unwrap());
            MsgBundle::new(MsgId::random(), entity_path, timepoint, splatted)
        });

        Ok(msgs)
    }
}

fn bundle_from_iter<'a, C: SerializableComponent>(
    data: impl IntoIterator<Item = &'a C>,
) -> Result<ComponentBundle, MsgBundleError> {
    // TODO(cmc): Eeeh, that's not ideal to repeat that kind of logic in here, but orphan rules
    // kinda force us to :/

    let array: Box<dyn Array> = TryIntoArrow::try_into_arrow(data)?;
    let wrapped = wrap_in_listarray(array);

    Ok(ComponentBundle::new(C::name(), wrapped))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let [standard, transforms, splats] = MsgSender::new("some/path").into_messages().unwrap();
        assert!(standard.is_none());
        assert!(transforms.is_none());
        assert!(splats.is_none());
    }

    #[test]
    fn full() -> Result<(), MsgSenderError> {
        let labels = vec![crate::Label("label1".into()), crate::Label("label2".into())];
        let transform = vec![crate::Transform::Rigid3(crate::Rigid3::default())];
        let color = crate::ColorRGBA::from_rgb(255, 0, 255);

        let [standard, transforms, splats] = MsgSender::new("some/path")
            .with_component(&labels)?
            .with_component(&transform)?
            .with_splat(color)?
            .into_messages()
            .unwrap();

        {
            let standard = standard.unwrap();
            let idx = standard.find_component(&crate::Label::name()).unwrap();
            let bundle = &standard.components[idx];
            assert!(bundle.num_rows() == 1);
            assert!(bundle.num_instances(0).unwrap() == 2);
        }

        {
            let transforms = transforms.unwrap();
            let idx = transforms
                .find_component(&crate::Transform::name())
                .unwrap();
            let bundle = &transforms.components[idx];
            assert!(bundle.num_rows() == 1);
            assert!(bundle.num_instances(0).unwrap() == 1);
        }

        {
            let splats = splats.unwrap();
            let idx = splats.find_component(&crate::ColorRGBA::name()).unwrap();
            let bundle = &splats.components[idx];
            assert!(bundle.num_rows() == 1);
            assert!(bundle.num_instances(0).unwrap() == 1);
        }

        Ok(())
    }

    #[test]
    fn timepoint_last_write_wins() {
        let my_timeline = Timeline::new("my_timeline", crate::TimeType::Sequence);
        let sender = MsgSender::new("some/path")
            .with_time(my_timeline, 0)
            .with_time(my_timeline, 1)
            .with_time(my_timeline, 2);
        assert_eq!(
            TimeInt::from(2),
            *sender.timepoint.get(&my_timeline).unwrap()
        );
    }

    #[test]
    fn timepoint_timeless() -> Result<(), MsgSenderError> {
        let my_timeline = Timeline::new("my_timeline", crate::TimeType::Sequence);

        let sender = MsgSender::new("some/path")
            .with_timeless(true)
            .with_component(&vec![crate::Label("label1".into())])?
            .with_time(my_timeline, 2);
        assert!(!sender.timepoint.is_empty()); // not yet

        let [standard, _, _] = sender.into_messages().unwrap();
        assert!(standard.unwrap().time_point.is_empty());

        Ok(())
    }

    #[test]
    fn attempted_batch() -> Result<(), MsgSenderError> {
        let res = MsgSender::new("some/path")
            .with_component(&vec![crate::Label("label1".into())])?
            .with_component(&vec![crate::Label("label2".into())])?
            .into_messages();

        let Err(MsgSenderError::MoreThanOneRow(err)) = res else { panic!() };
        assert_eq!([(crate::Label::name(), 2)].to_vec(), err);

        Ok(())
    }

    #[test]
    fn illegal_instance_key() -> Result<(), MsgSenderError> {
        let _ = MsgSender::new("some/path")
            .with_component(&vec![crate::Label("label1".into())])?
            .with_component(&vec![crate::InstanceKey(u64::MAX)])?
            .into_messages()?;

        // TODO(cmc): This is not detected as of today, but it probably should.

        Ok(())
    }

    #[test]
    fn splatted_instance_key() -> Result<(), MsgSenderError> {
        let res = MsgSender::new("some/path")
            .with_component(&vec![crate::Label("label1".into())])?
            .with_splat(crate::InstanceKey(42));

        assert!(matches!(res, Err(MsgSenderError::SplattedInstanceKeys)));

        Ok(())
    }
}
