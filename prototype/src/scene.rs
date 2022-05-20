use crate::*;

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
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

        for (type_path, _) in store.iter() {
            if type_path.last() == Some(&TypePathComponent::Name("pos".into())) {
                visit_data_and_siblings(
                    store,
                    time_query,
                    type_path,
                    ("radius",),
                    |pos: &[f32; 3], radius: Option<&f32>| {
                        slf.points.push(Point3 {
                            pos,
                            radius: radius.copied(),
                        });
                    },
                );
            }
        }

        slf
    }
}
