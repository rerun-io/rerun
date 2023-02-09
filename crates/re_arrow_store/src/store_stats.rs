use crate::{
    ComponentBucket, ComponentTable, DataStore, IndexBucket, IndexBucketIndices, IndexTable,
    PersistentComponentTable, PersistentIndexTable,
};

// ---

// TODO(cmc): count buckets?
// TODO(cmc): compute incrementally once/if this becomes too expensive.
#[derive(Default, Debug)]
pub struct DataStoreStats {
    pub total_timeless_index_rows: u64,
    pub total_timeless_index_size_bytes: u64,
    pub total_timeless_component_rows: u64,
    pub total_timeless_component_size_bytes: u64,

    pub total_temporal_index_rows: u64,
    pub total_temporal_index_size_bytes: u64,
    pub total_temporal_component_rows: u64,
    pub total_temporal_component_size_bytes: u64,

    pub total_index_rows: u64,
    pub total_index_size_bytes: u64,
    pub total_component_rows: u64,
    pub total_component_size_bytes: u64,
}

impl DataStoreStats {
    pub fn from_store(store: &DataStore) -> Self {
        crate::profile_function!();

        let total_timeless_index_rows = store.total_timeless_index_rows();
        let total_timeless_index_size_bytes = store.total_timeless_index_size_bytes();
        let total_timeless_component_rows = store.total_timeless_component_rows();
        let total_timeless_component_size_bytes = store.total_timeless_component_size_bytes();

        let total_temporal_index_rows = store.total_temporal_index_rows();
        let total_temporal_index_size_bytes = store.total_temporal_index_size_bytes();
        let total_temporal_component_rows = store.total_temporal_component_rows();
        let total_temporal_component_size_bytes = store.total_temporal_component_size_bytes();

        let total_index_rows = total_timeless_index_rows + total_temporal_index_rows;
        let total_index_size_bytes =
            total_timeless_index_size_bytes + total_temporal_index_size_bytes;
        let total_component_rows = total_timeless_component_rows + total_temporal_component_rows;
        let total_component_size_bytes =
            total_timeless_component_size_bytes + total_temporal_component_size_bytes;

        Self {
            total_timeless_index_rows,
            total_timeless_index_size_bytes,
            total_timeless_component_rows,
            total_timeless_component_size_bytes,
            total_temporal_index_rows,
            total_temporal_index_size_bytes,
            total_temporal_component_rows,
            total_temporal_component_size_bytes,
            total_index_rows,
            total_index_size_bytes,
            total_component_rows,
            total_component_size_bytes,
        }
    }
}

// --- Data store ---

impl DataStore {
    /// Returns the number of timeless index rows stored across this entire store, i.e. the sum of
    /// the number of rows across all of its timeless index tables.
    pub fn total_timeless_index_rows(&self) -> u64 {
        self.timeless_indices
            .values()
            .map(|table| table.total_rows())
            .sum()
    }

    /// Returns the size of the timeless index data stored across this entire store, i.e. the sum
    /// of the size of the data stored across all of its timeless index tables, in bytes.
    pub fn total_timeless_index_size_bytes(&self) -> u64 {
        self.timeless_indices
            .values()
            .map(|table| table.total_size_bytes())
            .sum()
    }

    /// Returns the number of timeless component rows stored across this entire store, i.e. the
    /// sum of the number of rows across all of its timeless component tables.
    pub fn total_timeless_component_rows(&self) -> u64 {
        self.timeless_components
            .values()
            .map(|table| table.total_rows())
            .sum()
    }

    /// Returns the size of the timeless component data stored across this entire store, i.e. the
    /// sum of the size of the data stored across all of its timeless component tables, in bytes.
    pub fn total_timeless_component_size_bytes(&self) -> u64 {
        self.timeless_components
            .values()
            .map(|table| table.total_size_bytes())
            .sum()
    }

    /// Returns the number of temporal index rows stored across this entire store, i.e. the sum of
    /// the number of rows across all of its temporal index tables.
    pub fn total_temporal_index_rows(&self) -> u64 {
        self.indices.values().map(|table| table.total_rows()).sum()
    }

    /// Returns the size of the temporal index data stored across this entire store, i.e. the sum
    /// of the size of the data stored across all of its temporal index tables, in bytes.
    pub fn total_temporal_index_size_bytes(&self) -> u64 {
        self.indices
            .values()
            .map(|table| table.total_size_bytes())
            .sum()
    }

    /// Returns the number of temporal component rows stored across this entire store, i.e. the
    /// sum of the number of rows across all of its temporal component tables.
    pub fn total_temporal_component_rows(&self) -> u64 {
        self.components
            .values()
            .map(|table| table.total_rows())
            .sum()
    }

    /// Returns the size of the temporal component data stored across this entire store, i.e. the
    /// sum of the size of the data stored across all of its temporal component tables, in bytes.
    pub fn total_temporal_component_size_bytes(&self) -> u64 {
        self.components
            .values()
            .map(|table| table.total_size_bytes())
            .sum()
    }
}

// --- Persistent Indices ---

impl PersistentIndexTable {
    /// Returns the number of rows stored across this table.
    pub fn total_rows(&self) -> u64 {
        self.num_rows
    }

    /// Returns the size of the data stored across this table, in bytes.
    pub fn total_size_bytes(&self) -> u64 {
        self.indices
            .values()
            .map(|index| std::mem::size_of_val(index.as_slice()) as u64)
            .sum::<u64>()
    }
}

// --- Indices ---

impl IndexTable {
    /// Returns the number of rows stored across this entire table, i.e. the sum of the number
    /// of rows stored across all of its buckets.
    pub fn total_rows(&self) -> u64 {
        self.buckets
            .values()
            .map(|bucket| bucket.total_rows())
            .sum()
    }

    /// Returns the size of data stored across this entire table, i.e. the sum of the size of
    /// the data stored across all of its buckets, in bytes.
    pub fn total_size_bytes(&self) -> u64 {
        self.buckets
            .values()
            .map(|bucket| bucket.total_size_bytes())
            .sum()
    }
}

impl IndexBucket {
    /// Returns the number of rows stored across this bucket.
    pub fn total_rows(&self) -> u64 {
        self.indices.read().times.len() as u64
    }

    /// Returns the size of the data stored across this bucket, in bytes.
    pub fn total_size_bytes(&self) -> u64 {
        let IndexBucketIndices {
            is_sorted: _,
            time_range: _,
            times,
            indices,
        } = &*self.indices.read();

        std::mem::size_of_val(times.as_slice()) as u64
            + indices
                .values()
                .map(|index| std::mem::size_of_val(index.as_slice()) as u64)
                .sum::<u64>()
    }
}

// --- Persistent Components ---

impl PersistentComponentTable {
    /// Returns the number of rows stored across this table.
    pub fn total_rows(&self) -> u64 {
        self.total_rows
    }

    /// Returns the size of the data stored across this table, in bytes.
    pub fn total_size_bytes(&self) -> u64 {
        self.total_size_bytes
    }
}

// --- Components ---

impl ComponentTable {
    /// Returns the number of rows stored across this entire table, i.e. the sum of the number
    /// of rows stored across all of its buckets.
    pub fn total_rows(&self) -> u64 {
        self.buckets.iter().map(|bucket| bucket.total_rows()).sum()
    }

    /// Returns the size of data stored across this entire table, i.e. the sum of the size of
    /// the data stored across all of its buckets, in bytes.
    pub fn total_size_bytes(&self) -> u64 {
        self.buckets
            .iter()
            .map(|bucket| bucket.total_size_bytes())
            .sum()
    }
}

impl ComponentBucket {
    /// Returns the number of rows stored across this bucket.
    pub fn total_rows(&self) -> u64 {
        self.total_rows
    }

    /// Returns the size of the data stored across this bucket, in bytes.
    pub fn total_size_bytes(&self) -> u64 {
        self.total_size_bytes
    }
}

// This test exists because the documentation and online discussions revolving around
// arrow2's `estimated_bytes_size()` function indicate that there's a lot of limitations and
// edge cases to be aware of.
//
// Also, it's just plain hard to be sure that the answer you get is the answer you're looking
// for with these kinds of tools. When in doubt.. test everything we're going to need from it.
//
// In many ways, this is a specification of what we mean when we ask "what's the size of this
// Arrow array?".
#[test]
#[allow(clippy::from_iter_instead_of_collect)]
fn test_arrow_estimated_size_bytes() {
    use arrow2::{
        array::{Array, Float64Array, ListArray, StructArray, UInt64Array, Utf8Array},
        compute::aggregate::estimated_bytes_size,
        datatypes::{DataType, Field},
        offset::Offsets,
    };

    // simple primitive array
    {
        let data = vec![42u64; 100];
        let array = UInt64Array::from_vec(data.clone()).boxed();
        assert_eq!(
            std::mem::size_of_val(data.as_slice()),
            estimated_bytes_size(&*array)
        );
    }

    // utf8 strings array
    {
        let data = vec![Some("some very, very, very long string indeed"); 100];
        let array = Utf8Array::<i32>::from(data.clone()).to_boxed();

        let raw_size_bytes = data
            .iter()
            // headers + bodies!
            .map(|s| std::mem::size_of_val(s) + std::mem::size_of_val(s.unwrap().as_bytes()))
            .sum::<usize>();
        let arrow_size_bytes = estimated_bytes_size(&*array);

        assert_eq!(5600, raw_size_bytes);
        assert_eq!(4404, arrow_size_bytes); // smaller because validity bitmaps instead of opts
    }

    // simple primitive list array
    {
        let data = std::iter::repeat(vec![42u64; 100])
            .take(50)
            .collect::<Vec<_>>();
        let array = {
            let array_flattened =
                UInt64Array::from_vec(data.clone().into_iter().flatten().collect()).boxed();

            ListArray::<i32>::new(
                ListArray::<i32>::default_datatype(DataType::UInt64),
                Offsets::try_from_lengths(std::iter::repeat(50).take(50))
                    .unwrap()
                    .into(),
                array_flattened,
                None,
            )
            .boxed()
        };

        let raw_size_bytes = data
            .iter()
            // headers + bodies!
            .map(|s| std::mem::size_of_val(s) + std::mem::size_of_val(s.as_slice()))
            .sum::<usize>();
        let arrow_size_bytes = estimated_bytes_size(&*array);

        assert_eq!(41200, raw_size_bytes);
        assert_eq!(40200, arrow_size_bytes); // smaller because smaller inner headers
    }

    // compound type array
    {
        #[derive(Clone, Copy)]
        struct Point {
            x: f64,
            y: f64,
        }
        impl Default for Point {
            fn default() -> Self {
                Self { x: 42.0, y: 666.0 }
            }
        }

        let data = vec![Point::default(); 100];
        let array = {
            let x = Float64Array::from_vec(data.iter().map(|p| p.x).collect()).boxed();
            let y = Float64Array::from_vec(data.iter().map(|p| p.y).collect()).boxed();
            let fields = vec![
                Field::new("x", DataType::Float64, false),
                Field::new("y", DataType::Float64, false),
            ];
            StructArray::new(DataType::Struct(fields), vec![x, y], None).boxed()
        };

        let raw_size_bytes = std::mem::size_of_val(data.as_slice());
        let arrow_size_bytes = estimated_bytes_size(&*array);

        assert_eq!(1600, raw_size_bytes);
        assert_eq!(1600, arrow_size_bytes);
    }

    // compound type list array
    {
        #[derive(Clone, Copy)]
        struct Point {
            x: f64,
            y: f64,
        }
        impl Default for Point {
            fn default() -> Self {
                Self { x: 42.0, y: 666.0 }
            }
        }

        let data = std::iter::repeat(vec![Point::default(); 100])
            .take(50)
            .collect::<Vec<_>>();
        let array: Box<dyn Array> = {
            let array = {
                let x =
                    Float64Array::from_vec(data.iter().flatten().map(|p| p.x).collect()).boxed();
                let y =
                    Float64Array::from_vec(data.iter().flatten().map(|p| p.y).collect()).boxed();
                let fields = vec![
                    Field::new("x", DataType::Float64, false),
                    Field::new("y", DataType::Float64, false),
                ];
                StructArray::new(DataType::Struct(fields), vec![x, y], None)
            };

            ListArray::<i32>::new(
                ListArray::<i32>::default_datatype(array.data_type().clone()),
                Offsets::try_from_lengths(std::iter::repeat(50).take(50))
                    .unwrap()
                    .into(),
                array.boxed(),
                None,
            )
            .boxed()
        };

        let raw_size_bytes = data
            .iter()
            // headers + bodies!
            .map(|s| std::mem::size_of_val(s) + std::mem::size_of_val(s.as_slice()))
            .sum::<usize>();
        let arrow_size_bytes = estimated_bytes_size(&*array);

        assert_eq!(81200, raw_size_bytes);
        assert_eq!(80200, arrow_size_bytes); // smaller because smaller inner headers
    }
}
