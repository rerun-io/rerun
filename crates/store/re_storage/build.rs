fn main() {
    tonic_build::compile_protos("proto/rerun/storage.proto").expect("compile protos failed");
}
