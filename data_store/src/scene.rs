use crate::*;

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Point3<'s> {
    pub pos: &'s [f32; 3],
    pub radius: Option<f32>,
}

#[derive(Default)]
pub struct Scene3D<'s> {
    /// The path is the parent path, e.g. `points[43]` containing `points[43].pos`, `points[43].radius` etc.
    pub points: BTreeMap<TypePath, Vec<Point3<'s>>>,
}

impl<'s> Scene3D<'s> {
    pub fn from_store<Time: 'static + Clone + Ord>(
        store: &'s TypePathDataStore<Time>,
        time_query: &TimeQuery<Time>,
    ) -> Self {
        let mut slf = Self::default();

        for (type_path, _) in store.iter() {
            if type_path.last() == Some(&TypePathComponent::String("pos".into())) {
                let mut points = vec![];
                visit_data_and_siblings(
                    store,
                    time_query,
                    type_path,
                    ("radius",),
                    |pos: &[f32; 3], radius: Option<&f32>| {
                        points.push(Point3 {
                            pos,
                            radius: radius.copied(),
                        });
                    },
                );
                slf.points.insert(parent(type_path), points);
            }
        }

        slf
    }
}

fn parent(type_path: &TypePath) -> TypePath {
    let mut type_path = type_path.clone();
    type_path.pop_back();
    type_path
}
