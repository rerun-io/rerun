use re_log_types::{component_types::InstanceKey, DataRow, DataTableError, RecordingId, RowId};

use crate::{
    log::DataCell,
    time::{Time, TimeInt, TimePoint, Timeline},
    Component, EntityPath, RecordingStream, SerializableComponent,
};

// ---

/// Errors that can occur when constructing or sending messages
/// using [`MsgSender`].
#[derive(thiserror::Error, Debug)]
pub enum MsgSenderError {
    /// Instance keys cannot be splatted
    #[error("Instance keys cannot be splatted")]
    SplattedInstanceKeys,

    /// Number of instances across components don't match
    #[error("Instance keys cannot be splatted")]
    MismatchNumberOfInstances,

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
///     rec_stream: &RecordingStream,
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
///         .send(rec_stream)
///         .map_err(Into::into)
/// }
/// ```
pub struct MsgSender {
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

    /// Read the file at the given path and log it.
    ///
    /// Supported file extensions are:
    ///  * `glb`, `gltf`, `obj`: encoded meshes, leaving it to the viewer to decode
    ///  * `jpg`, `jpeg`: encoded JPEG, leaving it to the viewer to decode. Requires the `image` feature.
    ///  * `png` and other image formats: decoded here. Requires the `image` feature.
    ///
    /// All other extensions will return an error.
    pub fn from_file_path(
        file_path: &std::path::Path,
    ) -> Result<Self, re_log_types::FromFileError> {
        let ent_path = re_log_types::EntityPath::from_file_path_as_single_string(file_path);
        let cell = DataCell::from_file_path(file_path)?;

        let mut timepoint = TimePoint::from([(Timeline::log_time(), Time::now().into())]);

        // This may sounds like a good idea, but that means `rerun *.jpg` will
        // actually act like it is playing a bunch of files over time, perhaps over many years.
        // Starting at the first time, this also means only one image is shown at the start.
        // We should probably just jump to the end of the stream or something,
        // but it is the eve of the 0.6.0 release, so TODO(emilk)
        let add_created_and_modified_timlines = false;
        if add_created_and_modified_timlines {
            if let Ok(metadata) = std::fs::metadata(file_path) {
                use re_log_types::TimeType;
                if let Ok(time) = metadata.created() {
                    if let Ok(time) = Time::try_from(time) {
                        timepoint.insert(Timeline::new("created", TimeType::Time), time.into());
                    }
                }
                if let Ok(time) = metadata.modified() {
                    if let Ok(time) = Time::try_from(time) {
                        timepoint.insert(Timeline::new("modified", TimeType::Time), time.into());
                    }
                }
            }
        }

        Ok(Self {
            num_instances: Some(cell.num_instances()),
            instanced: vec![cell],
            timepoint,
            ..Self::new(ent_path)
        })
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
    pub fn with_component<'a, C: SerializableComponent>(
        self,
        data: impl IntoIterator<Item = &'a C>,
    ) -> Result<Self, MsgSenderError> {
        let cell = DataCell::try_from_native(data).map_err(DataTableError::from)?;
        self.with_cell(cell)
    }

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
    pub fn with_cell(mut self, cell: DataCell) -> Result<MsgSender, MsgSenderError> {
        let num_instances = cell.num_instances();

        if let Some(cur_num_instances) = self.num_instances {
            if cur_num_instances != num_instances {
                if num_instances == 1 {
                    self.splatted.push(cell);
                    return Ok(self);
                } else {
                    return Err(MsgSenderError::MismatchNumberOfInstances);
                }
            }
        } else {
            // If this is the first appended collection, it gets to decide the row-length
            // (i.e. number of instances) of all future collections.
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
    pub fn send(self, rec_stream: &RecordingStream) -> Result<(), DataTableError> {
        if !rec_stream.is_enabled() {
            return Ok(()); // silently drop the message
        }

        let [row_standard, row_splats] = self.into_rows();

        if let Some(row_splats) = row_splats {
            rec_stream.record_row(row_splats);
        }

        // Always the primary component last so range-based queries will include the other data.
        // Since the primary component can't be splatted it must be in msg_standard, see(#1215).
        if let Some(row_standard) = row_standard {
            rec_stream.record_row(row_standard);
        }

        Ok(())
    }

    /// Turns the current message into a single [`re_log_types::LogMsg`]
    pub fn into_log_msg(
        self,
        recording_id: RecordingId,
    ) -> Result<re_log_types::LogMsg, DataTableError> {
        let data_table = re_log_types::DataTable::from_rows(
            re_log_types::TableId::random(),
            self.into_rows().into_iter().flatten(),
        );
        let arrow_msg = data_table.to_arrow_msg()?;
        Ok(re_log_types::LogMsg::ArrowMsg(recording_id, arrow_msg))
    }

    fn into_rows(self) -> [Option<DataRow>; 2] {
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

        let mut rows = [(); 2].map(|_| None);

        // Standard
        rows[0] = (!instanced.is_empty()).then(|| {
            DataRow::from_cells(
                RowId::random(),
                timepoint.clone(),
                entity_path.clone(),
                num_instances.unwrap_or(0),
                instanced,
            )
        });

        // Splats
        // TODO(#1629): unsplit splats once new data cells are in
        rows[1] = (!splatted.is_empty()).then(|| {
            splatted.push(DataCell::from_native(&[InstanceKey::SPLAT]));
            DataRow::from_cells(RowId::random(), timepoint, entity_path, 1, splatted)
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
        let [standard, splats] = MsgSender::new("some/path").into_rows();
        assert!(standard.is_none());
        assert!(splats.is_none());
    }

    #[test]
    fn full() -> Result<(), MsgSenderError> {
        let labels = vec![
            components::Label("label1".into()),
            components::Label("label2".into()),
        ];
        let transform = vec![components::Transform3D::IDENTITY];
        let color = components::ColorRGBA::from_rgb(255, 0, 255);

        let [standard, splats] = MsgSender::new("some/path")
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
            let splats = splats.unwrap();

            let idx = splats.find_cell(&components::Transform3D::name()).unwrap();
            let cell = &splats.cells[idx];
            assert!(cell.num_instances() == 1);

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

        let [standard, _] = sender.into_rows();
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

    #[test]
    fn num_instances_mismatch() -> Result<(), MsgSenderError> {
        // 1 for 1 -- fine
        {
            MsgSender::new("some/path")
                .with_component([components::Label("label1".into())].as_slice())?
                .with_component([components::ColorRGBA::from_rgb(1, 1, 1)].as_slice())?;
        }

        // 3 for 1 -- fine, implicit splat
        {
            MsgSender::new("some/path")
                .with_component(
                    [
                        components::Label("label1".into()),
                        components::Label("label2".into()),
                        components::Label("label3".into()),
                    ]
                    .as_slice(),
                )?
                .with_component([components::ColorRGBA::from_rgb(1, 1, 1)].as_slice())?;
        }

        // 3 for 2 -- nope, makes no sense
        {
            let res = MsgSender::new("some/path")
                .with_component(
                    [
                        components::Label("label1".into()),
                        components::Label("label2".into()),
                        components::Label("label3".into()),
                    ]
                    .as_slice(),
                )?
                .with_component(
                    [
                        components::ColorRGBA::from_rgb(1, 1, 1),
                        components::ColorRGBA::from_rgb(1, 1, 1),
                    ]
                    .as_slice(),
                );

            assert!(matches!(
                res,
                Err(MsgSenderError::MismatchNumberOfInstances)
            ));
        }

        Ok(())
    }
}
