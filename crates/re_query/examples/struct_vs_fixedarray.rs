use arrow2::array::Array;
use arrow2_convert::deserialize::*;
use arrow2_convert::serialize::*;
use arrow2_convert::{ArrowDeserialize, ArrowField, ArrowSerialize};

use re_log_types::field_types::FixedSizeArrayField;
use re_log_types::msg_bundle::Component;

#[derive(Clone, Copy, Debug, ArrowField, ArrowSerialize, ArrowDeserialize, PartialEq)]
pub struct Point3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Copy, Debug, ArrowField, ArrowSerialize, ArrowDeserialize, PartialEq)]
pub struct Point3DFlattened(#[arrow_field(type = "FixedSizeArrayField<f32,3>")] pub [f32; 3]);

fn main() {
    const N: usize = 400_000; // roughly a frame of nyud

    {
        let points = vec![
            Point3D {
                x: 42.0,
                y: 420.0,
                z: 4200.0,
            };
            N
        ];

        let now = std::time::Instant::now();
        let mut count = 0usize;
        for p in points {
            count += 1;
        }
        eprintln!("iterated {count} native points in {:?}", now.elapsed());
    }

    eprintln!("---");

    {
        let points = vec![
            Point3D {
                x: 42.0,
                y: 420.0,
                z: 4200.0,
            };
            N
        ];

        let now = std::time::Instant::now();
        let arr: Box<dyn Array> = points.try_into_arrow().unwrap();
        eprintln!(
            "serialized {} arrow struct points in {:?}",
            arr.len(),
            now.elapsed()
        );

        let now = std::time::Instant::now();
        let mut count = 0usize;
        let iter = arrow_array_deserialize_iterator::<Option<Point3D>>(&*arr).unwrap();
        for p in iter {
            count += 1;
        }
        eprintln!(
            "iterated {count} arrow struct points in {:?}",
            now.elapsed()
        );
    }

    eprintln!("---");

    {
        let points = vec![Point3DFlattened([42.0, 420.0, 4200.0]); N];

        let now = std::time::Instant::now();
        let arr: Box<dyn Array> = points.try_into_arrow().unwrap();
        dbg!(arr.data_type());
        eprintln!(
            "serialized {} arrow flat points in {:?}",
            arr.len(),
            now.elapsed()
        );

        {
            let iter = arrow_array_deserialize_iterator::<Point3DFlattened>(&*arr).unwrap();
            let now = std::time::Instant::now();
            let mut count = 0usize;
            for p in iter {
                count += 1;
            }
            eprintln!("iterated {count} arrow flat points in {:?}", now.elapsed());
            // unsafe {
            //     let N1 = re_log_types::field_types::N1;
            //     let N2 = re_log_types::field_types::N2;
            //     eprintln!("step 1 ({N1}) in {:?}", re_log_types::field_types::STEP1);
            //     eprintln!("step 2 ({N2}) in {:?}", re_log_types::field_types::STEP2);
            // }
        }
    }
}
