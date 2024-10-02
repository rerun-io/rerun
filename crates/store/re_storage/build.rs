fn main() {
    // FIXME (zehiko) We should not use build.rs, but instead we should have a small binary that
    // we call from pixi and this binary calls tonic build prost stuff. Resulting code should be committed
    // along with the spec file. We should also ensure that prost uses protoc from the pixi env.
    tonic_build::compile_protos("proto/rerun/v0/storage.proto").expect("compile protos failed");
}
