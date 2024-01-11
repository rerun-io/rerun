#include <openMVG/image/image_io.hpp>
#include <openMVG/sfm/sfm.hpp>
#include <openMVG/sfm/sfm_data.hpp>
#include <openMVG/sfm/sfm_data_io.hpp>
#include <openMVG/sfm/sfm_data_utils.hpp>
#include <rerun.hpp>

using namespace std::string_literals;

int main(int argc, char* argv[]) {
    if (argc < 2) {
        std::cout << "Enter the sfm_data file path\n";
        return EXIT_FAILURE;
    }
    // load sfm_data from pre-saved json file
    openMVG::sfm::SfM_Data sfm_data;
    openMVG::sfm::Load(sfm_data, static_cast<std::string>(argv[1]), openMVG::sfm::ESfM_Data::ALL);

    // rerun_sdk
    const auto rec = rerun::RecordingStream("openMVG_sfm_data_visualization");
    rec.spawn().exit_on_failure();

    const auto& poses_container = sfm_data.GetPoses();
    for (const auto& [view_id, view] : sfm_data.views) {
        // be sure that the view->s_Img_path is the full path of the image when sfm_data got exported
        const auto& view_file_name = view->s_Img_path;
        const auto id_intrinsic = view->id_intrinsic;
        const auto id_pose = view->id_pose;

        if (auto it = poses_container.find(id_pose); it != poses_container.end()) {
            const auto& view_pose = poses_container.at(id_pose);
            Eigen::Vector3d translation = -view_pose.rotation() * view_pose.center();

            rerun::datatypes::Mat3x3 rr_rotation{
                {static_cast<float>(view_pose.rotation()(0, 0)),
                 static_cast<float>(view_pose.rotation()(1, 0)),
                 static_cast<float>(view_pose.rotation()(2, 0)),
                 static_cast<float>(view_pose.rotation()(0, 1)),
                 static_cast<float>(view_pose.rotation()(1, 1)),
                 static_cast<float>(view_pose.rotation()(2, 1)),
                 static_cast<float>(view_pose.rotation()(0, 2)),
                 static_cast<float>(view_pose.rotation()(1, 2)),
                 static_cast<float>(view_pose.rotation()(2, 2))}
            };

            rerun::datatypes::Vec3D rr_translation{
                static_cast<float>(translation(0)),
                static_cast<float>(translation(1)),
                static_cast<float>(translation(2))
            };

            rec.log(
                "world/camera/"s + view_file_name,
                rerun::archetypes::Transform3D(
                    rerun::datatypes::TranslationAndMat3x3(rr_translation, rr_rotation, true)
                )
            );
            const rerun::datatypes::Vec2D resolution{
                static_cast<float>(view->ui_width),
                static_cast<float>(view->ui_height)
            };
            rec.log(
                "world/camera/"s + view_file_name,
                rerun::archetypes::Pinhole::from_focal_length_and_resolution(
                    sfm_data.GetIntrinsics().at(id_intrinsic)->getParams()[0],
                    resolution
                )
            );
            openMVG::image::Image<openMVG::image::RGBColor> img;
            auto is_img_loaded = openMVG::image::ReadImage(view->s_Img_path.c_str(), &img);
            if (is_img_loaded) {
                rec.log(
                    "world/camera/"s + view_file_name,
                    rerun::Image(
                        {static_cast<uint64_t>(img.rows()),
                         static_cast<uint64_t>(img.cols()),
                         static_cast<uint64_t>(img.Depth())},
                        img.GetMat().data()->data()
                    )
                );
            }
        }
    }
    const auto& landmarks = sfm_data.GetLandmarks();
    std::vector<rerun::components::Position3D> points3d;
    std::vector<rerun::components::KeypointId> track_ids;
    std::unordered_map<uint32_t, std::vector<rerun::components::Position2D>> points2d_per_img;
    for (const auto& landmark : landmarks) {
        points3d.push_back(rerun::components::Position3D(
            landmark.second.X(0),
            landmark.second.X(1),
            landmark.second.X(2)
        ));
        track_ids.push_back(landmark.first);
        for (const auto& obs : landmark.second.obs) {
            points2d_per_img[obs.first].push_back(
                {static_cast<float>(obs.second.x(0)), static_cast<float>(obs.second.x(1))}
            );
        }
    }
    rec.log("world/3Dpoints"s, rerun::archetypes::Points3D(points3d).with_keypoint_ids(track_ids));

    for (const auto& view : sfm_data.views) {
        rec.log(
            "world/camera"s + view.second->s_Img_path,
            rerun::archetypes::Points2D(points2d_per_img.at(view.second->id_view))
        );
    }
    return 0;
}
