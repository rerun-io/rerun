fn main() {
    tonic_build::compile_protos("proto/rerun/v0/storage.proto").expect("compile protos failed");
}
