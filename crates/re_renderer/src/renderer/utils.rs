// part of std, but unstable https://github.com/rust-lang/rust/issues/88581
pub const fn next_multiple_of(cur: u32, rhs: u32) -> u32 {
    match cur % rhs {
        0 => cur,
        r => cur + (rhs - r),
    }
}
