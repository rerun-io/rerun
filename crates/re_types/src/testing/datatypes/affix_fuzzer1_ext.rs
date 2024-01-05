use super::AffixFuzzer1;

// TODO(#4690): this should be codegen'd.
impl crate::SizeBytes for AffixFuzzer1 {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            single_float_optional,
            single_string_required,
            single_string_optional,
            many_floats_optional,
            many_strings_required,
            many_strings_optional,
            flattened_scalar,
            almost_flattened_scalar,
            from_parent,
        } = self;
        single_float_optional.heap_size_bytes()
            + single_string_required.heap_size_bytes()
            + single_string_optional.heap_size_bytes()
            + many_floats_optional.heap_size_bytes()
            + many_strings_required.heap_size_bytes()
            + many_strings_optional.heap_size_bytes()
            + flattened_scalar.heap_size_bytes()
            + almost_flattened_scalar.heap_size_bytes()
            + from_parent.heap_size_bytes()
    }
}
