use crate::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point3<'s> {
    pub pos: &'s [f32; 3],
    pub radius: Option<f32>,
}

#[derive(Default)]
pub struct Scene3D<'s> {
    pub points: Vec<Point3<'s>>,
}

impl<'s> Scene3D<'s> {
    pub fn from_store(store: &'s DataStore, time_query: &TimeQuery) -> Self {
        let mut slf = Self::default();

        for (type_path, data) in store.iter() {
            if type_path.last() == Some(&TypePathComponent::Name("pos".into())) {
                if let Some(pos_data) = data.read::<[f32; 3]>() {
                    Self::collect_points(&mut slf.points, store, time_query, type_path, pos_data);
                }
            }
        }

        slf
    }

    fn collect_points(
        out_points: &mut Vec<Point3<'s>>,
        store: &'s DataStore,
        time_query: &TimeQuery,
        type_path: &TypePath,
        pos_data: &'s DataPerTypePath<[f32; 3]>,
    ) {
        let radius_path = sibling(type_path, "radius");

        match pos_data {
            DataPerTypePath::Individual(pos) => {
                let radius_reader = IndividualDataReader::<f32>::new(store, &radius_path);
                for (index_path, values_over_time) in pos.iter() {
                    query(values_over_time, time_query, |time, pos| {
                        out_points.push(Point3 {
                            pos,
                            radius: radius_reader.latest_at(index_path, time).copied(),
                        });
                    });
                }
            }
            DataPerTypePath::Batched(pos) => {
                for (index_path_prefix, pos) in pos.iter() {
                    let radius_store = store.get::<f32>(&radius_path);
                    query(pos, time_query, |time, pos| {
                        let radius_reader =
                            BatchedDataReader::new(radius_store, index_path_prefix, time);
                        for (index_path_suffix, pos) in pos {
                            out_points.push(Point3 {
                                pos,
                                radius: radius_reader.latest_at(index_path_suffix).copied(),
                            });
                        }
                    });
                }
            }
        }
    }
}

fn sibling(type_path: &TypePath, name: &str) -> TypePath {
    let mut type_path = type_path.clone();
    type_path.pop_back();
    type_path.push_back(TypePathComponent::Name(name.into()));
    type_path
}
