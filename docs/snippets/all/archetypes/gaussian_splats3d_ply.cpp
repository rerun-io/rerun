// Log a 3D Gaussian Splatting (3DGS) PLY file.

#include <rerun.hpp>

#include <iostream>

int main(int argc, char* argv[]) {
    if (argc < 2) {
        std::cerr << "Usage: " << argv[0] << " <path_to_splats.ply>"
                  << std::endl;
        return 1;
    }

    const auto path = argv[1];

    const auto rec =
        rerun::RecordingStream("rerun_example_gaussian_splats3d_ply");
    rec.spawn().exit_on_failure();

    rec.log_file_from_path(path);
}
