use re_log_types::{component_types::InstanceKey, DataRow, DataTableError};

use nohash_hasher::IntMap;

use crate::{
    components::Transform,
    log::{DataCell, LogMsg, MsgId},
    sink::LogSink,
    time::{Time, TimeInt, TimePoint, Timeline},
    Component, ComponentName, EntityPath, SerializableComponent,
};

// TODO(#1619): Rust SDK batching

// ---

/// Errors that can occur when constructing or sending messages
/// using [`MsgSender`].
#[derive(thiserror::Error, Debug)]
pub enum MsgSenderError {
    /// Instance keys cannot be splatted
    #[error("Instance keys cannot be splatted")]
    SplattedInstanceKeys,

    /// [`InstanceKey`] with a [`u64::MAX`] was found, but is reserved for Rerun internals.
    #[error("InstanceKey(u64::MAX) is reserved for Rerun internals")]
    IllegalInstanceKey,

    /// A message during packing. See [`DataTableError`].
    #[error(transparent)]
    PackingError(#[from] DataTableError),
}

/// Facilitates building and sending component payloads with the Rerun SDK.
///
/// ```ignore
/// fn log_coordinate_space(
///     session: &Session,
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
// TODO(#1619): this whole thing needs to be rethought to incorporate batching and datatables.
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
    num_instances: Option<u32>,

    /// All the instanced component collections that have been appended to this message.
    ///
    /// As of today, they must have exactly 1 row of data (no batching), which itself must have
    /// `Self::num_instances` instance keys.
    instanced: Vec<DataCell>,

    /// All the splatted components that have been appended to this message.
    ///
    /// By definition, all `DataCell`s in this vector will have 1 row (no batching) and more
    /// importantly a single, special instance key for that row.
    splatted: Vec<DataCell>,
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
        let cell = DataCell::try_from_native(data).map_err(DataTableError::from)?;

        let num_instances = cell.num_instances();

        // If this is the first appended collection, it gets to decide the row-length (i.e. number
        // of instances) of all future collections.
        if self.num_instances.is_none() {
            self.num_instances = Some(num_instances);
        }

        // TODO(cmc): if this is an InstanceKey and it contains u64::MAX, fire IllegalInstanceKey.

        self.instanced.push(cell);

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

        self.splatted
            .push(DataCell::try_from_native(&[data]).map_err(DataTableError::from)?);

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

    /// Consumes, packs, sanity checks and finally sends the message to the currently configured
    /// target of the SDK.
    pub fn send(self, sink: &impl std::borrow::Borrow<dyn LogSink>) -> Result<(), DataTableError> {
        self.send_to_sink(sink.borrow())
    }

    /// Consumes, packs, sanity checks and finally sends the message to the currently configured
    /// target of the SDK.
    fn send_to_sink(self, sink: &dyn LogSink) -> Result<(), DataTableError> {
        if !sink.is_enabled() {
            return Ok(()); // silently drop the message
        }

        let [row_standard, row_transforms, row_splats] = self.into_rows();

        if let Some(row_transforms) = row_transforms {
            sink.send(LogMsg::ArrowMsg((&row_transforms.into_table()).try_into()?));
        }
        if let Some(row_splats) = row_splats {
            sink.send(LogMsg::ArrowMsg((&row_splats.into_table()).try_into()?));
        }
        // Always the primary component last so range-based queries will include the other data.
        // Since the primary component can't be splatted it must be in msg_standard, see(#1215).
        if let Some(row_standard) = row_standard {
            sink.send(LogMsg::ArrowMsg((&row_standard.into_table()).try_into()?));
        }

        Ok(())
    }

    fn into_rows(self) -> [Option<DataRow>; 3] {
        let Self {
            entity_path,
            timepoint,
            timeless,
            num_instances,
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
        let mut all_cells: Vec<_> = instanced.into_iter().map(Some).collect();
        let standard_cells: Vec<_> = all_cells
            .iter_mut()
            .filter(|cell| cell.as_ref().unwrap().component_name() != Transform::name())
            .map(|cell| cell.take().unwrap())
            .collect();
        let transform_cells: Vec<_> = all_cells
            .iter_mut()
            .filter(|cell| {
                cell.as_ref()
                    .map_or(false, |cell| cell.component_name() == Transform::name())
            })
            .map(|cell| cell.take().unwrap())
            .collect();
        debug_assert!(all_cells.into_iter().all(|cell| cell.is_none()));

        // sanity check: no row-level batching
        let mut rows_per_comptype: IntMap<ComponentName, usize> = IntMap::default();
        for cell in standard_cells
            .iter()
            .chain(&transform_cells)
            .chain(&splatted)
        {
            *rows_per_comptype.entry(cell.component_name()).or_default() += 1;
        }

        // sanity check: transforms can't handle multiple instances
        let num_transform_instances = transform_cells
            .get(0)
            .map_or(0, |cell| cell.num_instances());
        if num_transform_instances > 1 {
            re_log::warn!("detected Transform component with multiple instances");
        }

        let mut rows = [(); 3].map(|_| None);

        // Standard
        rows[0] = (!standard_cells.is_empty()).then(|| {
            DataRow::from_cells(
                MsgId::random(),
                timepoint.clone(),
                entity_path.clone(),
                num_instances.unwrap_or(0),
                standard_cells,
            )
        });

        // Transforms
        rows[1] = (!transform_cells.is_empty()).then(|| {
            DataRow::from_cells(
                MsgId::random(),
                timepoint.clone(),
                entity_path.clone(),
                num_transform_instances,
                transform_cells,
            )
        });

        // Splats
        // TODO(cmc): unsplit splats once new data cells are in
        rows[2] = (!splatted.is_empty()).then(|| {
            splatted.push(DataCell::from_native(&[InstanceKey::SPLAT]));
            DataRow::from_cells(MsgId::random(), timepoint, entity_path, 1, splatted)
        });

        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{components, time};

    #[test]
    fn empty() {
        let [standard, transforms, splats] = MsgSender::new("some/path").into_rows();
        assert!(standard.is_none());
        assert!(transforms.is_none());
        assert!(splats.is_none());
    }

    #[test]
    fn full() -> Result<(), MsgSenderError> {
        let labels = vec![
            components::Label("label1".into()),
            components::Label("label2".into()),
        ];
        let transform = vec![components::Transform::Rigid3(components::Rigid3::default())];
        let color = components::ColorRGBA::from_rgb(255, 0, 255);

        let [standard, transforms, splats] = MsgSender::new("some/path")
            .with_component(&labels)?
            .with_component(&transform)?
            .with_splat(color)?
            .into_rows();

        {
            let standard = standard.unwrap();
            let idx = standard.find_cell(&components::Label::name()).unwrap();
            let cell = &standard.cells[idx];
            assert!(cell.num_instances() == 2);
        }

        {
            let transforms = transforms.unwrap();
            let idx = transforms
                .find_cell(&components::Transform::name())
                .unwrap();
            let cell = &transforms.cells[idx];
            assert!(cell.num_instances() == 1);
        }

        {
            let splats = splats.unwrap();
            let idx = splats.find_cell(&components::ColorRGBA::name()).unwrap();
            let cell = &splats.cells[idx];
            assert!(cell.num_instances() == 1);
        }

        Ok(())
    }

    #[test]
    fn timepoint_last_write_wins() {
        let my_timeline = Timeline::new("my_timeline", time::TimeType::Sequence);
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
        let my_timeline = Timeline::new("my_timeline", time::TimeType::Sequence);

        let sender = MsgSender::new("some/path")
            .with_timeless(true)
            .with_component([components::Label("label1".into())].as_slice())?
            .with_time(my_timeline, 2);
        assert!(!sender.timepoint.is_empty()); // not yet

        let [standard, _, _] = sender.into_rows();
        assert!(standard.unwrap().timepoint.is_empty());

        Ok(())
    }

    #[test]
    fn illegal_instance_key() -> Result<(), MsgSenderError> {
        let _ = MsgSender::new("some/path")
            .with_component([components::Label("label1".into())].as_slice())?
            .with_component([components::InstanceKey(u64::MAX)].as_slice())?
            .into_rows();

        // TODO(cmc): This is not detected as of today, but it probably should.

        Ok(())
    }

    #[test]
    fn splatted_instance_key() -> Result<(), MsgSenderError> {
        let res = MsgSender::new("some/path")
            .with_component([components::Label("label1".into())].as_slice())?
            .with_splat(components::InstanceKey(42));

        assert!(matches!(res, Err(MsgSenderError::SplattedInstanceKeys)));

        Ok(())
    }
}
