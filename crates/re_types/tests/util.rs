#[allow(dead_code)]
pub fn assert_extensions(array: &dyn ::arrow2::array::Array, expected: &[&str]) {
    let mut extracted = vec![];
    extract_extensions(array.data_type(), &mut extracted);
    similar_asserts::assert_eq!(expected, extracted);
}

#[allow(dead_code)]
pub fn extract_extensions(datatype: &::arrow2::datatypes::DataType, acc: &mut Vec<String>) {
    match datatype {
        arrow2::datatypes::DataType::List(field)
        | arrow2::datatypes::DataType::FixedSizeList(field, _)
        | arrow2::datatypes::DataType::LargeList(field) => {
            extract_extensions(field.data_type(), acc);
        }
        arrow2::datatypes::DataType::Struct(fields) => {
            for field in fields.iter() {
                extract_extensions(field.data_type(), acc);
            }
        }
        arrow2::datatypes::DataType::Extension(fqname, inner, _) => {
            acc.push(fqname.clone());
            extract_extensions(inner, acc);
        }
        _ => {}
    }
}
