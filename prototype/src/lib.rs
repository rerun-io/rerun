use std::collections::BTreeMap;

use nohash_hasher::IntMap;

const SNAPSHOTS: bool = false;

pub enum AtomType {
    // 1D:
    I32,
    F32,

    Color,

    // ----------------------------
    // 2D:
    Pos2,
    BBox2D,
    LineSegment2D,
    Image,

    // ----------------------------
    // 3D:
    Pos3,
    BBox3,
    Path3D,
    LineSegment3D,
    Mesh3D,
    Camera,

    // ----------------------------
    // N-D:
    Vecf32,
}

pub enum Data {
    F32(f32),
    Pos3([f32; 3]),
}

pub enum StructType {
    /// ```ignore
    /// pos: Pos3,
    /// radius: Option<F32>,
    /// color: Option<Color>,
    /// ```
    Point3D,
}

type SpaceId = String; // TODO
type TimeSource = String; // TODO

// pub struct TypePath(Vec<TypePathComponent>);
pub type TypePath = im::Vector<TypePathComponent>;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypePathComponent {
    /// Struct member
    Name(String),

    /// Table (array/map) member.
    /// Tables are homogenous, so it is the same type path for all.
    Index,
}

// pub struct DataPath(Vec<DataPathComponent>);
pub type DataPath = im::Vector<DataPathComponent>;

pub fn into_type_path(data_path: DataPath) -> (TypePath, IndexPathKey) {
    let mut type_path = im::Vector::default();
    let mut index_path = IndexPathKey::default();
    for component in data_path {
        match component {
            DataPathComponent::Name(name) => {
                type_path.push_back(TypePathComponent::Name(name));
            }
            DataPathComponent::Index(index) => {
                type_path.push_back(TypePathComponent::Index);
                index_path.push_back(&index);
            }
        }
    }
    (type_path, index_path)
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DataPathComponent {
    /// struct member
    Name(String),

    /// array/table/map member
    Index(Index),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Index {
    /// For arrays, assumed to be dense (0, 1, 2, …)
    Sequence(u64),

    /// X,Y pixel coordinates, from top left.
    Pixel([u64; 2]),

    /// Any integer, e.g. a hash
    Integer(i128),

    /// UUID/GUID
    // Uuid(Uuid),

    /// Anything goes
    String(String),

    /// A child that only exists for one single instant.
    /// The same path does not refer to the same object next instant.
    Temporary(u64),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialOrd, Ord)]
pub struct IndexPathKey {
    hashes: [u64; 2],
}

impl IndexPathKey {
    pub fn push_back(&mut self, comp: &Index) {
        self.hashes = [
            hash_with_seed(&comp, ((self.hashes[0] as u128) << 64) | 123),
            hash_with_seed(&comp, ((self.hashes[1] as u128) << 64) | 456),
        ];
    }
}

impl std::cmp::PartialEq for IndexPathKey {
    fn eq(&self, other: &Self) -> bool {
        self.hashes == other.hashes // much faster, and extremely low chance of collision
    }
}

impl std::hash::Hash for IndexPathKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.hashes[0]);
    }
}

impl nohash_hasher::IsEnabled for IndexPathKey {}

/// Hash the given value.
#[inline]
fn hash_with_seed(value: impl std::hash::Hash, seed: u128) -> u64 {
    use std::hash::Hasher as _;
    let mut hasher = ahash::AHasher::new_with_keys(123, seed);
    value.hash(&mut hasher);
    hasher.finish()
}

#[derive(Default)]
pub struct DataTree {
    /// If this is a struct
    named_children: BTreeMap<String, DataTree>,

    /// If this is a table (array, map, …)
    /// Note: always homogenous!
    indexed_children: Option<Box<DataTree>>,

    // /// e.g. Point3
    // typ: Option<StructType>,

    // /// Could be e.g. "world" for a `Point3`, or "world.z` for `Point3.x`.
    // space: Option<SpaceId>,

    // /// If this is a leaf/atom.
    // atom_type: Option<AtomType>,
    /// If this is a leaf/atom
    data_history: DataHistory,
}

impl DataTree {
    pub fn insert(
        &mut self,
        mut type_path: TypePath,
        index_path: IndexPathKey,
        time_stamp: TimeStamp,
        data: Data,
    ) {
        if let Some(first) = type_path.pop_front() {
            match first {
                TypePathComponent::Name(name) => self
                    .named_children
                    .entry(name)
                    .or_default()
                    .insert(type_path, index_path, time_stamp, data),
                TypePathComponent::Index => self
                    .indexed_children
                    .get_or_insert_with(Default::default)
                    .insert(type_path, index_path, time_stamp, data),
            }
        } else {
            self.data_history.insert(index_path, time_stamp, data);
        }
    }
}

/// For a specific [`TypePath`].
///
/// type-erased version of [`DataHistoryT`].
#[derive(Default)]
pub struct DataHistory(Option<Box<dyn std::any::Any>>);

impl DataHistory {
    pub fn read<T: 'static>(&self) -> Option<&DataHistoryT<T>> {
        self.0
            .as_ref()
            .and_then(|any| any.downcast_ref::<DataHistoryT<T>>())
    }

    pub fn write<T: 'static>(&mut self) -> Option<&mut DataHistoryT<T>> {
        self.0
            .get_or_insert_with(|| Box::new(DataHistoryT::<T>::default()))
            .downcast_mut::<DataHistoryT<T>>()
    }

    pub fn insert(&mut self, index_path: IndexPathKey, time_stamp: TimeStamp, data: Data) {
        match data {
            Data::F32(value) => {
                if let Some(data_store) = self.write::<f32>() {
                    data_store.insert(index_path, time_stamp, value);
                } else {
                    // TODO: log warning
                }
            }
            Data::Pos3(value) => {
                if let Some(data_store) = self.write::<[f32; 3]>() {
                    data_store.insert(index_path, time_stamp, value);
                } else {
                    // TODO: log warning
                }
            }
        }
    }
}

/// For a specific [`TypePath`].
pub struct DataHistoryT<T> {
    per_time_source: BTreeMap<TimeSource, DataPerTimeSource<T>>,
}

impl<T> Default for DataHistoryT<T> {
    fn default() -> Self {
        Self {
            per_time_source: Default::default(),
        }
    }
}

impl<T: Clone> DataHistoryT<T> {
    pub fn insert(&mut self, index_path: IndexPathKey, time_stamp: TimeStamp, data: T) {
        // TODO: optimize away clones for the case when time_stamp.0.len() == 1
        for (time_source, time_value) in time_stamp.0 {
            self.per_time_source.entry(time_source).or_default().insert(
                index_path,
                time_value,
                data.clone(),
            );
        }
    }
}

struct DataPerTimeSource<T> {
    /// fast to find latest value at a certain time.
    values: IntMap<IndexPathKey, BTreeMap<TimeValue, T>>,

    // for each new time slice, what is new?
    new_per_time: BTreeMap<TimeValue, DataPerTime<T>>,

    everything_per_time: BTreeMap<
        TimeValue,
        im::HashMap<IndexPathKey, T, nohash_hasher::BuildNoHashHasher<IndexPathKey>>,
        // im::OrdMap<IndexPathKey, T>,
        // nohash_hasher::IntMap<IndexPathKey, T>,
    >,
}

impl<T> Default for DataPerTimeSource<T> {
    fn default() -> Self {
        Self {
            values: Default::default(),
            new_per_time: Default::default(),
            everything_per_time: Default::default(),
        }
    }
}

impl<T: Clone> DataPerTimeSource<T> {
    pub fn insert(&mut self, index_path: IndexPathKey, data_time: TimeValue, data: T) {
        if SNAPSHOTS {
            // these will be needed to handle the case of adding to old times
            // self.new_per_time
            //     .entry(data_time)
            //     .or_default()
            //     .values
            //     .insert(index_path, data.clone());

            let last = self.everything_per_time.iter().rev().next();
            match last {
                None => {
                    self.everything_per_time
                        .insert(data_time, Default::default());
                }
                Some((state_time, state)) => {
                    if state_time < &data_time {
                        let new_state = state.clone();
                        self.everything_per_time.insert(data_time, new_state);
                    } else if state_time == &data_time {
                        // OK
                    } else {
                        unimplemented!("You must add data in chronological order. Sorry");
                    }
                }
            };

            self.everything_per_time
                .get_mut(&data_time)
                .unwrap()
                .insert(index_path, data);
        } else {
            self.values
                .entry(index_path)
                .or_default()
                .insert(data_time, data.clone());
        }
    }
}

struct DataPerTime<T> {
    /// New values set this at this time.
    values: IntMap<IndexPathKey, T>,
}

impl<T> Default for DataPerTime<T> {
    fn default() -> Self {
        Self {
            values: Default::default(),
        }
    }
}

#[inline(never)] // better profiling
fn latest_at<'data, T>(
    data_over_time: &'data BTreeMap<TimeValue, T>,
    time: &'_ TimeValue,
) -> Option<(&'data TimeValue, &'data T)> {
    data_over_time.range(..=time).rev().next()
}

#[inline(never)] // better profiling
fn values_in_range<'data, T>(
    data_over_time: &'data BTreeMap<TimeValue, T>,
    time_range: &'_ TimeRange,
) -> impl Iterator<Item = (&'data TimeValue, &'data T)> {
    data_over_time.range(time_range.min..=time_range.max)
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum TimeValue {
    // Time(u64), // TODO
    Sequence(i64),
}

/// A point in time.
///
/// It can be represented by [`Time`], a sequence index, or a mix of several things.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeStamp(pub BTreeMap<String, TimeValue>);

#[derive(Clone, Copy, Debug)]
pub struct TimeRange {
    pub min: TimeValue,
    pub max: TimeValue,
}

pub enum TimeSelector {
    LatestAt(TimeSource, TimeValue),
    Range(TimeSource, TimeRange),
}

pub fn visit_tree(
    mut path: TypePath,
    root: &DataTree,
    visitor: &mut impl FnMut(&TypePath, &DataTree),
) {
    visitor(&path, root);
    for (name, tree) in &root.named_children {
        path.push_back(TypePathComponent::Name(name.to_string()));
        visit_tree(path.clone(), tree, visitor);
        path.pop_back();
    }
    if let Some(indexed_children) = &root.indexed_children {
        path.push_back(TypePathComponent::Index);
        visit_tree(path.clone(), indexed_children, visitor);
        path.pop_back();
    }
}

#[derive(Default)]
pub struct Scene3D {
    pub points: Vec<Point3>,
}

impl Scene3D {
    pub fn from_tree(root: &DataTree, time_selector: &TimeSelector) -> Self {
        let mut scene = Scene3D::default();
        visit_tree(TypePath::default(), root, &mut |path, tree| {
            if let Some(point3) = Point3History::from_tree(tree) {
                point3.read(time_selector, |point3| {
                    scene.points.push(point3);
                });
            }
        });
        scene
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point3 {
    pos: [f32; 3],
    radius: Option<f32>,
}

pub struct Point3History<'a> {
    pos: &'a DataHistoryT<[f32; 3]>,
    radius: Option<&'a DataHistoryT<f32>>,
}

impl<'a> Point3History<'a> {
    pub fn from_tree(tree: &'a DataTree) -> Option<Self> {
        let pos = tree
            .named_children
            .get("pos")?
            .data_history
            .read::<[f32; 3]>()?;

        let radius = tree
            .named_children
            .get("radius")
            .and_then(|v| v.data_history.read::<f32>());

        Some(Self { pos, radius })
    }

    #[inline(never)] // better profiling
    pub fn read(&self, selector: &TimeSelector, visitor: impl FnMut(Point3)) {
        match selector {
            TimeSelector::LatestAt(time_source, time_value) => {
                let pos = self.pos.per_time_source.get(time_source);
                let radius = self
                    .radius
                    .and_then(|radius| radius.per_time_source.get(time_source));

                if let Some(pos) = pos {
                    Self::visit_latest_at(time_value, pos, radius, visitor);
                }
            }
            TimeSelector::Range(time_source, time_range) => {
                let pos = self.pos.per_time_source.get(time_source);
                let radius = self
                    .radius
                    .and_then(|radius| radius.per_time_source.get(time_source));

                if let Some(pos) = pos {
                    Self::visit_over_time(time_range, pos, radius, visitor);
                }
            }
        }
    }

    #[inline(never)] // better profiling
    fn visit_latest_at<'b>(
        time: &TimeValue,
        pos: &'b DataPerTimeSource<[f32; 3]>,
        radius: Option<&'b DataPerTimeSource<f32>>,
        mut visitor: impl FnMut(Point3),
    ) {
        if SNAPSHOTS {
            if let Some((_, pos)) = latest_at(&pos.everything_per_time, time) {
                let radius = radius
                    .and_then(|radius| latest_at(&radius.everything_per_time, time))
                    .map(|(_, x)| x);
                for (index_path, pos) in pos {
                    visitor(Point3 {
                        pos: *pos,
                        radius: radius.and_then(|v| v.get(index_path)).copied(),
                    });
                }
            }
        } else {
            for (index_path, pos) in &pos.values {
                if let Some((_, pos)) = latest_at(pos, time) {
                    visitor(Point3 {
                        pos: *pos,
                        radius: radius
                            .and_then(|v| v.values.get(index_path))
                            .and_then(|v| latest_at(v, time))
                            .map(|(_, x)| *x),
                    });
                }
            }
        }
    }

    fn visit_over_time<'b>(
        time_range: &TimeRange,
        pos: &'b DataPerTimeSource<[f32; 3]>,
        radius: Option<&'b DataPerTimeSource<f32>>,
        mut visitor: impl FnMut(Point3),
    ) {
        if SNAPSHOTS {
            unimplemented!()
        } else {
            for (index_path, pos) in &pos.values {
                for (pos_time, pos) in values_in_range(pos, time_range) {
                    let radius = radius
                        .and_then(|v| v.values.get(index_path))
                        .and_then(|v| latest_at(v, pos_time));
                    // let radius = radius.filter(|(radius_time, _)| time_range.min <= radius_time); // allow attributes from before the range!
                    let radius = radius.map(|(_, x)| *x);
                    visitor(Point3 { pos: *pos, radius });
                }
            }
        }
    }
}

#[test]
fn test_data_storage() {
    fn time_stamp(seq: i64) -> TimeStamp {
        let mut time_stamp = TimeStamp::default();
        time_stamp
            .0
            .insert("frame".to_string(), TimeValue::Sequence(seq));
        time_stamp
    }
    fn data_path(index: u64, field: &str) -> DataPath {
        im::vector![
            DataPathComponent::Name("camera".into()),
            DataPathComponent::Index(Index::String("left".into())),
            DataPathComponent::Name("point".into()),
            DataPathComponent::Index(Index::Sequence(index)),
            DataPathComponent::Name(field.into()),
        ]
    }

    let mut tree = DataTree::default();

    let (type_path, index_path) = into_type_path(data_path(0, "pos"));
    tree.insert(
        type_path,
        index_path,
        time_stamp(1),
        Data::Pos3([1.0, 2.0, 3.0]),
    );

    let (type_path, index_path) = into_type_path(data_path(0, "radius"));
    tree.insert(type_path, index_path, time_stamp(2), Data::F32(1.0));

    assert_eq!(
        Scene3D::from_tree(
            &tree,
            &TimeSelector::LatestAt("frame".to_string(), TimeValue::Sequence(0)),
        )
        .points,
        vec![]
    );

    assert_eq!(
        Scene3D::from_tree(
            &tree,
            &TimeSelector::LatestAt("frame".to_string(), TimeValue::Sequence(1)),
        )
        .points,
        vec![Point3 {
            pos: [1.0, 2.0, 3.0],
            radius: None
        }]
    );

    assert_eq!(
        Scene3D::from_tree(
            &tree,
            &TimeSelector::LatestAt("frame".to_string(), TimeValue::Sequence(2)),
        )
        .points,
        vec![Point3 {
            pos: [1.0, 2.0, 3.0],
            radius: Some(1.0)
        }]
    );
}
