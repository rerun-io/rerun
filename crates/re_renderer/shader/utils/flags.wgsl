fn has_any_flag(flags: u32, flags_to_check: u32) -> bool {
    return (flags & flags_to_check) > 0u;
}
