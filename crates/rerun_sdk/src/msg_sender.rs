use arrow2::array::Array;
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
        "All component collections must share the same row-length (i.e. number of instances),\
            got {0:?} instead"
    )]
    MismatchedRowLengths(Vec<(ComponentName, usize)>),
    #[error("Instance keys cannot be splatted")]
    SplattedInstanceKeys,
    #[error(transparent)]
    PackingError(#[from] MsgBundleError),
}

// TODO: tests:
// - instanced
// - splat
// - timeless
// - error paths

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
    nb_instances: Option<usize>,
    /// All the instanced component collections that have been appended to this message.
    ///
    /// As of today, they must have exactly 1 row of data (no batching), which itself must have
    /// `Self::nb_instances` instance keys.
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

            nb_instances: None,
            instanced: Vec::new(),
            splatted: Vec::new(),
        }
    }

    // --- Time ---

    // TODO: test last write wins

    /// Appends a given `timepoint` to the current message.
    ///
    /// This can be called any number of times. In case of collisions, last write wins.
    /// I.e. if `timepoint` contains a timestamp `ts1` for a timeline `my_time` and the current
    /// message already has a timestamp `ts0` for that same timeline, then the new value (`ts1`)
    /// overwrite the existing value (`ts0`).
    ///
    /// `MsgSender` automatically keeps track of the logging time, which is recorded when
    /// [`Self::new`] is first called.
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

        let nb_instances = bundle.nb_instances(0).unwrap(); // must have exactly 1 row atm

        // If this is the first appended collection, it gets to decide the row-length (i.e. number
        // of instances) of all future collections.
        if self.nb_instances.is_none() {
            self.nb_instances = Some(nb_instances);
        }

        // Detect mismatched row-lengths early on... unless it's a Transform bundle: transforms
        // behave differently and will be sent in their own message!
        if C::name() != Transform::name() && self.nb_instances.unwrap() != nb_instances {
            let collections = self
                .instanced
                .into_iter()
                .map(|bundle| (bundle.name, bundle.nb_instances(0).unwrap_or(0)))
                .collect();
            return Err(MsgSenderError::MismatchedRowLengths(collections));
        }

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
        let Self {
            entity_path,
            timepoint,
            timeless,
            nb_instances: _,
            instanced,
            mut splatted,
        } = self;

        // TODO(cmc): The sanity checks we do in here can (and probably should) be done in
        // `MsgBundle` instead so that the python SDK benefits from them too... but one step at a
        // time.

        // TODO: at this point, should have the same number of rows all over the place.
        // TODO: at this point, transform should be neither splat nor length >1

        let timepoint = if timeless { [].into() } else { timepoint };

        // TODO(cmc): just use `Vec::drain_filter` once it goes stable...
        let mut all_bundles: Vec<_> = instanced.into_iter().map(Some).collect();
        let standard_bundles: Vec<_> = all_bundles
            .iter_mut()
            .filter(|bundle| bundle.as_ref().unwrap().name != Transform::name())
            .map(|bundle| bundle.take().unwrap())
            .collect();
        let transform_bundles: Vec<_> = all_bundles
            .iter_mut()
            .filter(|bundle| {
                bundle
                    .as_ref()
                    .map_or(false, |bundle| bundle.name == Transform::name())
            })
            .map(|bundle| bundle.take().unwrap())
            .collect();
        debug_assert!(all_bundles.into_iter().all(|bundle| bundle.is_none()));

        // Standard & transforms
        for bundles in [standard_bundles, transform_bundles] {
            let msg = MsgBundle::new(
                MsgId::random(),
                entity_path.clone(),
                timepoint.clone(),
                bundles,
            );
            session.send(LogMsg::ArrowMsg(msg.try_into()?));
        }

        // Splats
        {
            splatted.push(bundle_from_iter(&[InstanceKey::SPLAT])?);

            let msg = MsgBundle::new(MsgId::random(), entity_path, timepoint, splatted);
            session.send(LogMsg::ArrowMsg(msg.try_into()?));
        }

        Ok(())
    }
}

fn bundle_from_iter<'a, C: SerializableComponent>(
    data: impl IntoIterator<Item = &'a C>,
) -> Result<ComponentBundle, MsgBundleError> {
    // TODO(cmc): Eeeh, that's not ideal to repeat that kind of logic in here, but orphan rules
    // kinda force us to :/

    let array: Box<dyn Array> = TryIntoArrow::try_into_arrow(data)?;
    let wrapped = wrap_in_listarray(array).boxed();

    Ok(ComponentBundle::new(C::name(), wrapped))
}
