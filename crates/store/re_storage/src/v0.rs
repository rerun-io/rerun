use re_log_types::ResolvedTimeRange;

tonic::include_proto!("rerun.storage.v0");

impl From<ResolvedTimeRange> for TimeRange {
    fn from(rtr: ResolvedTimeRange) -> Self {
        Self {
            start: rtr.min().as_i64(),
            end: rtr.max().as_i64(),
        }
    }
}
