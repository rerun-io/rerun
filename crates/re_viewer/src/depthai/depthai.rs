use itertools::Itertools;
use re_log_types::{EntityPath, EntityPathHash};
use std::collections::{BTreeSet, HashMap};

use super::super::ui::SpaceView;
use super::api::BackendCommChannel;
use super::ws::WsMessageData;
use instant::Instant;
use std::fmt;

use strum::EnumIter;
use strum::IntoEnumIterator;

#[derive(serde::Deserialize, serde::Serialize, fmt::Debug, PartialEq, Clone, Copy, EnumIter)]
#[allow(non_camel_case_types)]
pub enum ColorCameraResolution {
    THE_720_P,
    THE_800_P,
    THE_1440X1080,
    THE_1080_P,
    THE_1200_P,
    THE_5_MP,
    THE_4_K,
    THE_12_MP,
    THE_4000X3000,
    THE_13_MP,
    THE_48_MP,
}

#[derive(serde::Deserialize, serde::Serialize, fmt::Debug, PartialEq, Clone, Copy, EnumIter)]
#[allow(non_camel_case_types)]
pub enum MonoCameraResolution {
    THE_400_P,
    THE_480_P,
    THE_720_P,
    THE_800_P,
    THE_1200_P,
}

// fmt::Display is used in UI while fmt::Debug is used with the depthai backend api
impl fmt::Display for ColorCameraResolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::THE_1080_P => write!(f, "1080p"),
            Self::THE_4_K => write!(f, "4k"),
            Self::THE_720_P => write!(f, "720p"),
            Self::THE_800_P => write!(f, "800p"),
            Self::THE_1200_P => write!(f, "1200p"),
            Self::THE_5_MP => write!(f, "5MP"),
            Self::THE_12_MP => write!(f, "12MP"),
            Self::THE_13_MP => write!(f, "13MP"),
            Self::THE_4000X3000 => write!(f, "4000x3000"),
            Self::THE_48_MP => write!(f, "48MP"),
            Self::THE_1440X1080 => write!(f, "1440x1080"),
        }
    }
}

impl fmt::Display for MonoCameraResolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::THE_400_P => write!(f, "400p"),
            Self::THE_480_P => write!(f, "480p"),
            Self::THE_720_P => write!(f, "720p"),
            Self::THE_800_P => write!(f, "800p"),
            Self::THE_1200_P => write!(f, "1200p"),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq)]
pub struct ColorCameraConfig {
    pub fps: u8,
    pub resolution: ColorCameraResolution,
    #[serde(rename = "xout_video")]
    pub stream_enabled: bool,
}

impl Default for ColorCameraConfig {
    fn default() -> Self {
        Self {
            fps: 30,
            resolution: ColorCameraResolution::THE_1080_P,
            stream_enabled: true,
        }
    }
}

impl fmt::Debug for ColorCameraConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Color camera config: fps: {}, resolution: {:?}",
            self.fps, self.resolution
        )
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq, EnumIter, Debug)]
#[allow(non_camel_case_types)]
pub enum BoardSocket {
    AUTO,
    RGB,
    LEFT,
    RIGHT,
    CENTER,
    CAM_A,
    CAM_B,
    CAM_C,
    CAM_D,
    CAM_E,
    CAM_F,
    CAM_G,
    CAM_H,
}

impl Default for BoardSocket {
    fn default() -> Self {
        Self::AUTO
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq)]
pub struct MonoCameraConfig {
    pub fps: u8,
    pub resolution: MonoCameraResolution,
    pub board_socket: BoardSocket,
    #[serde(rename = "xout")]
    pub stream_enabled: bool,
}

impl Default for MonoCameraConfig {
    fn default() -> Self {
        Self {
            fps: 30,
            resolution: MonoCameraResolution::THE_800_P,
            board_socket: BoardSocket::AUTO,
            stream_enabled: false,
        }
    }
}

impl fmt::Debug for MonoCameraConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Mono camera config: fps: {}, resolution: {:?}",
            self.fps, self.resolution
        )
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq)]
#[allow(non_camel_case_types)]
pub enum DepthProfilePreset {
    HIGH_DENSITY,
    HIGH_ACCURACY,
}

impl Default for DepthProfilePreset {
    fn default() -> Self {
        Self::HIGH_DENSITY
    }
}

impl fmt::Display for DepthProfilePreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HIGH_DENSITY => write!(f, "High Density"),
            Self::HIGH_ACCURACY => write!(f, "High Accuracy"),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq, Debug, EnumIter)]
#[allow(non_camel_case_types)]
pub enum DepthMedianFilter {
    MEDIAN_OFF,
    KERNEL_3x3,
    KERNEL_5x5,
    KERNEL_7x7,
}

impl Default for DepthMedianFilter {
    fn default() -> Self {
        Self::KERNEL_7x7
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq, Debug)]
pub struct DepthConfig {
    pub median: DepthMedianFilter,
    pub lr_check: bool,
    pub lrc_threshold: u64,
    pub extended_disparity: bool,
    pub subpixel_disparity: bool,
    pub sigma: i64,
    pub confidence: i64,
    pub align: BoardSocket,
}

impl Default for DepthConfig {
    fn default() -> Self {
        Self {
            median: DepthMedianFilter::default(),
            lr_check: true,
            lrc_threshold: 5,
            extended_disparity: false,
            subpixel_disparity: true,
            sigma: 0,
            confidence: 230,
            align: BoardSocket::RGB,
        }
    }
}

impl DepthConfig {
    pub fn default_as_option() -> Option<Self> {
        Some(Self::default())
    }
}

#[derive(Default, serde::Deserialize, serde::Serialize, Clone)]
pub struct DeviceConfig {
    pub color_camera: ColorCameraConfig,
    pub left_camera: MonoCameraConfig,
    pub right_camera: MonoCameraConfig,
    #[serde(default = "bool_true")]
    pub depth_enabled: bool, // Much easier to have an explicit bool for checkbox
    #[serde(default = "DepthConfig::default_as_option")]
    pub depth: Option<DepthConfig>,
    pub ai_model: AiModel,
}

impl PartialEq for DeviceConfig {
    fn eq(&self, other: &Self) -> bool {
        let depth_eq = match (&self.depth, &other.depth) {
            (Some(a), Some(b)) => a == b,
            _ => true, // If one is None, it's only different if depth_enabled is different
        };
        self.color_camera == other.color_camera
            && self.left_camera == other.left_camera
            && self.right_camera == other.right_camera
            && depth_eq
            && self.depth_enabled == other.depth_enabled
            && self.ai_model == other.ai_model
    }
}

#[inline]
fn bool_true() -> bool {
    true
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct DeviceConfigState {
    pub config: DeviceConfig,
    #[serde(skip)]
    pub update_in_progress: bool,
}

impl fmt::Debug for DeviceConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Device config: {:?} {:?} {:?}, depth: {:?}, ai_model: {:?}, depth_enabled: {:?}",
            self.color_camera,
            self.left_camera,
            self.right_camera,
            self.depth,
            self.ai_model,
            self.depth_enabled
        )
    }
}

#[derive(serde::Deserialize)]
struct PipelineResponse {
    message: String,
}

impl Default for PipelineResponse {
    fn default() -> Self {
        Self {
            message: "Pipeline not started".to_string(),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq, fmt::Debug)]
pub enum ErrorAction {
    None,
    FullReset,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, PartialEq, fmt::Debug)]
pub struct Error {
    pub action: ErrorAction,
    pub message: String,
}

impl Default for Error {
    fn default() -> Self {
        Self {
            action: ErrorAction::None,
            message: String::from("Invalid message"),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, PartialEq, fmt::Debug, Default)]
pub struct Device {
    pub id: DeviceId,
    pub supported_color_resolutions: Vec<ColorCameraResolution>,
    pub supported_left_mono_resolutions: Vec<MonoCameraResolution>,
    pub supported_right_mono_resolutions: Vec<MonoCameraResolution>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone, fmt::Debug)]
pub struct AiModel {
    pub path: String,
    pub display_name: String,
}

impl Default for AiModel {
    fn default() -> Self {
        Self {
            path: String::from(""),
            display_name: String::from("No model selected"),
        }
    }
}

impl PartialEq for AiModel {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct State {
    #[serde(skip)]
    devices_available: Option<Vec<DeviceId>>,
    #[serde(skip)]
    pub selected_device: Device,
    pub applied_device_config: DeviceConfigState,
    pub modified_device_config: DeviceConfigState,
    #[serde(skip, default = "all_subscriptions")]
    // Want to resubscribe to api when app is reloaded
    pub subscriptions: Vec<ChannelId>, // Shown in ui
    #[serde(skip)]
    setting_subscriptions: bool,
    #[serde(skip)]
    pub backend_comms: BackendCommChannel,
    #[serde(skip)]
    poll_instant: Option<Instant>,
    #[serde(default = "default_neural_networks")]
    pub neural_networks: Vec<AiModel>,
}

#[inline]
fn all_subscriptions() -> Vec<ChannelId> {
    ChannelId::iter().collect_vec()
}

#[inline]
fn default_neural_networks() -> Vec<AiModel> {
    vec![
        AiModel::default(),
        AiModel {
            path: String::from("yolo-v3-tiny-tf"),
            display_name: String::from("Yolo (tiny)"),
        },
        AiModel {
            path: String::from("mobilenet-ssd"),
            display_name: String::from("MobileNet SSD"),
        },
        AiModel {
            path: String::from("face-detection-retail-0004"),
            display_name: String::from("Face Detection"),
        },
        AiModel {
            path: String::from("age-gender-recognition-retail-0013"),
            display_name: String::from("Age gender recognition"),
        },
    ]
}

impl Default for State {
    fn default() -> Self {
        Self {
            devices_available: None,
            selected_device: Device::default(),
            applied_device_config: DeviceConfigState::default(),
            modified_device_config: DeviceConfigState::default(),
            subscriptions: ChannelId::iter().collect(),
            setting_subscriptions: false,
            backend_comms: BackendCommChannel::default(),
            poll_instant: Some(Instant::now()), // No default for Instant
            neural_networks: default_neural_networks(),
        }
    }
}

#[repr(u8)]
#[derive(
    serde::Serialize, serde::Deserialize, Copy, Clone, PartialEq, Eq, fmt::Debug, Hash, EnumIter,
)]
pub enum ChannelId {
    ColorImage,
    LeftMono,
    RightMono,
    DepthImage,
    PinholeCamera,
    ImuData,
}

/// Entity paths for depthai-viewer space views
/// !---- These have to match with EntityPath in rerun_py/rerun_sdk/depthai_viewer_backend/sdk_callbacks.py ----!
pub mod entity_paths {
    use lazy_static::lazy_static;
    use re_log_types::EntityPath;

    lazy_static! {
        pub static ref RGB_PINHOLE_CAMERA: EntityPath = EntityPath::from("color/camera/rgb");
        pub static ref LEFT_PINHOLE_CAMERA: EntityPath = EntityPath::from("mono/camera/left_mono");
        pub static ref LEFT_CAMERA_IMAGE: EntityPath =
            EntityPath::from("mono/camera/left_mono/Left mono");
        pub static ref RIGHT_PINHOLE_CAMERA: EntityPath =
            EntityPath::from("mono/camera/right_mono");
        pub static ref RIGHT_CAMERA_IMAGE: EntityPath =
            EntityPath::from("mono/camera/right_mono/Right mono");
        pub static ref RGB_CAMERA_IMAGE: EntityPath =
            EntityPath::from("color/camera/rgb/Color camera");
        pub static ref DETECTIONS: EntityPath = EntityPath::from("color/camera/rgb/Detections");
        pub static ref DETECTION: EntityPath = EntityPath::from("color/camera/rgb/Detection");
        pub static ref RGB_CAMERA_TRANSFORM: EntityPath = EntityPath::from("color/camera");
        pub static ref MONO_CAMERA_TRANSFORM: EntityPath = EntityPath::from("mono/camera");

        // --- These are extra for the depthai-viewer ---
        pub static ref COLOR_CAM_3D: EntityPath = EntityPath::from("color");
        pub static ref MONO_CAM_3D: EntityPath = EntityPath::from("mono");
        pub static ref DEPTH_RGB: EntityPath = EntityPath::from("color/camera/rgb/Depth");
        pub static ref DEPTH_LEFT_MONO: EntityPath =
            EntityPath::from("mono/camera/left_mono/Depth");
        pub static ref DEPTH_RIGHT_MONO: EntityPath =
            EntityPath::from("mono/camera/right_mono/Depth");
    }
}

impl State {
    /// Should the space view be added to the UI based on the new subscriptions (a subscription change occurred)
    fn create_entity_paths_from_subscriptions(
        &mut self,
        new_subscriptions: &Vec<ChannelId>,
    ) -> Vec<EntityPath> {
        let mut new_entity_paths = Vec::new();
        for channel in new_subscriptions.iter() {
            match channel {
                ChannelId::ColorImage => {
                    new_entity_paths.push(EntityPath::from("color/camera/rgb/Color camera"));
                }
                ChannelId::LeftMono => {
                    new_entity_paths.push(EntityPath::from("mono/camera/left_mono/Left mono"));
                }
                ChannelId::RightMono => {
                    new_entity_paths.push(EntityPath::from("mono/camera/right_mono/Right mono"));
                }
                ChannelId::DepthImage => {
                    new_entity_paths.push(EntityPath::from("color/camera/rgb/Depth"));
                    new_entity_paths.push(EntityPath::from("mono/camera/right_mono/Depth"));
                    new_entity_paths.push(EntityPath::from("mono/camera/left_mono/Depth"));
                }
                _ => {}
            }
        }
        new_entity_paths
    }

    /// Get the entities that should be removed based on UI (e.g. if depth is disabled, remove the depth image)
    pub fn get_entities_to_remove(&mut self) -> Vec<EntityPath> {
        let mut remove_entities = Vec::new();
        if self.applied_device_config.config.depth.is_none() {
            remove_entities.push(entity_paths::DEPTH_LEFT_MONO.clone());
            remove_entities.push(entity_paths::DEPTH_RIGHT_MONO.clone());
            remove_entities.push(entity_paths::DEPTH_RGB.clone());
        }
        if !self
            .applied_device_config
            .config
            .right_camera
            .stream_enabled
        {
            remove_entities.push(entity_paths::RIGHT_PINHOLE_CAMERA.clone());
            remove_entities.push(entity_paths::RIGHT_CAMERA_IMAGE.clone());
            // Both cams disabled -> remove 3D view
            if !self.applied_device_config.config.left_camera.stream_enabled {
                remove_entities.push(entity_paths::MONO_CAM_3D.clone());
                remove_entities.push(entity_paths::MONO_CAMERA_TRANSFORM.clone());
            }
        }
        if !self.applied_device_config.config.left_camera.stream_enabled {
            remove_entities.push(entity_paths::LEFT_PINHOLE_CAMERA.clone());
            remove_entities.push(entity_paths::LEFT_CAMERA_IMAGE.clone());
        }
        if !self
            .applied_device_config
            .config
            .color_camera
            .stream_enabled
        {
            remove_entities.push(entity_paths::RGB_PINHOLE_CAMERA.clone());
            remove_entities.push(entity_paths::RGB_CAMERA_IMAGE.clone());
            remove_entities.push(entity_paths::COLOR_CAM_3D.clone());
            remove_entities.push(entity_paths::RGB_CAMERA_TRANSFORM.clone());
        }
        if self.applied_device_config.config.ai_model.path.is_empty() {
            remove_entities.push(entity_paths::DETECTIONS.clone());
            remove_entities.push(entity_paths::DETECTION.clone());
        }
        remove_entities
    }

    /// DEPRECATED! Just keep it in the code as a reminder of how to do it
    /// Probably won't be needed when we make the move away from log_db in the future, will likely be implemented (much) differently.
    /// Until then we just loose a bit of performance if a user isn't viewing all of the channels
    /// Set subscriptions based on the visible UI
    // pub fn set_subscriptions_from_space_views(&mut self, visible_space_views: Vec<&SpaceView>) {
    //     // If any bool in the vec is true, the channel is currently visible in the ui somewhere
    //     let mut visibilities = HashMap::<ChannelId, Vec<bool>>::from([
    //         (ChannelId::ColorImage, Vec::new()),
    //         (ChannelId::LeftMono, Vec::new()),
    //         (ChannelId::RightMono, Vec::new()),
    //         (ChannelId::DepthImage, Vec::new()),
    //     ]);
    //     // Fill in visibilities
    //     for space_view in visible_space_views.iter() {
    //         let property_map = space_view.data_blueprint.data_blueprints_projected();
    //         for entity_path in space_view.data_blueprint.entity_paths().iter() {
    //             if let Some(channel_id) = DEPTHAI_ENTITY_HASHES.get(&entity_path.hash()) {
    //                 if let Some(visibility) = visibilities.get_mut(channel_id) {
    //                     visibility.push(property_map.get(entity_path).visible);
    //                 }
    //             }
    //         }
    //     }

    //     // First add subscriptions that don't have explicit enable disable buttons in the ui
    //     let mut possible_subscriptions = Vec::<ChannelId>::from([ChannelId::ImuData]);
    //     // Now add subscriptions that should be possible in terms of ui
    //     if self.applied_device_config.config.depth_enabled {
    //         possible_subscriptions.push(ChannelId::DepthImage);
    //     }
    //     if self
    //         .applied_device_config
    //         .config
    //         .color_camera
    //         .stream_enabled
    //     {
    //         possible_subscriptions.push(ChannelId::ColorImage);
    //     }

    //     if self.applied_device_config.config.left_camera.stream_enabled {
    //         possible_subscriptions.push(ChannelId::LeftMono);
    //     }
    //     if self
    //         .applied_device_config
    //         .config
    //         .right_camera
    //         .stream_enabled
    //     {
    //         possible_subscriptions.push(ChannelId::RightMono);
    //     }

    //     // Filter visibilities, include those that are currently visible and also possible (example pointcloud enabled == pointcloud possible)
    //     let mut subscriptions = visibilities
    //         .iter()
    //         .filter_map(|(channel, vis)| {
    //             if vis.iter().any(|x| *x) {
    //                 if possible_subscriptions.contains(channel) {
    //                     return Some(*channel);
    //                 }
    //             }
    //             None
    //         })
    //         .collect_vec();

    //     // Keep subscriptions that should be visible but have not yet been sent by the backend
    //     for channel in ChannelId::iter() {
    //         if !subscriptions.contains(&channel)
    //             && possible_subscriptions.contains(&channel)
    //             && self.subscriptions.contains(&channel)
    //         {
    //             subscriptions.push(channel);
    //         }
    //     }

    //     self.set_subscriptions(&subscriptions);
    // }

    pub fn set_subscriptions(&mut self, subscriptions: &Vec<ChannelId>) {
        if self.subscriptions.len() == subscriptions.len()
            && self
                .subscriptions
                .iter()
                .all(|channel_id| subscriptions.contains(channel_id))
        {
            return;
        }
        self.backend_comms.set_subscriptions(subscriptions);
        self.subscriptions = subscriptions.clone();
    }

    pub fn get_devices(&mut self) -> Vec<DeviceId> {
        // Return stored available devices or fetch them from the api (they get fetched every 30s via poller)
        if let Some(devices) = self.devices_available.clone() {
            return devices;
        }
        Vec::new()
    }

    pub fn shutdown(&mut self) {
        self.backend_comms.shutdown();
    }

    pub fn update(&mut self) {
        if let Some(ws_message) = self.backend_comms.receive() {
            re_log::debug!("Received message: {:?}", ws_message);
            match ws_message.data {
                WsMessageData::Subscriptions(subscriptions) => {
                    re_log::debug!("Setting subscriptions");
                    self.subscriptions = subscriptions;
                }
                WsMessageData::Devices(devices) => {
                    re_log::debug!("Setting devices...");
                    self.devices_available = Some(devices);
                }
                WsMessageData::Pipeline(config) => {
                    let mut subs = self.subscriptions.clone();
                    if config.depth.is_some() {
                        subs.push(ChannelId::DepthImage);
                    }
                    if config.color_camera.stream_enabled {
                        subs.push(ChannelId::ColorImage);
                    }
                    if config.left_camera.stream_enabled {
                        subs.push(ChannelId::LeftMono);
                    }
                    if config.right_camera.stream_enabled {
                        subs.push(ChannelId::RightMono);
                    }
                    self.applied_device_config.config = config.clone();
                    self.modified_device_config.config = config;
                    self.applied_device_config.config.depth_enabled =
                        self.applied_device_config.config.depth.is_some();
                    self.modified_device_config.config.depth_enabled =
                        self.modified_device_config.config.depth.is_some();
                    self.set_subscriptions(&subs);
                    self.applied_device_config.update_in_progress = false;
                }
                WsMessageData::Device(device) => {
                    re_log::debug!("Setting device: {device:?}");
                    self.selected_device = device;
                    self.backend_comms.set_subscriptions(&self.subscriptions);
                    self.backend_comms
                        .set_pipeline(&self.applied_device_config.config);
                    self.applied_device_config.update_in_progress = true;
                }
                WsMessageData::Error(error) => {
                    re_log::error!("Error: {:}", error.message);
                    self.applied_device_config.update_in_progress = false;
                    match error.action {
                        ErrorAction::None => (),
                        ErrorAction::FullReset => {
                            self.set_device("".into());
                        }
                    }
                }
                _ => {}
            }
        }

        if let Some(poll_instant) = self.poll_instant {
            if poll_instant.elapsed().as_secs() < 2 {
                return;
            }
            if self.selected_device.id == "" {
                self.backend_comms.get_devices();
            }
            self.poll_instant = Some(Instant::now());
        } else {
            self.poll_instant = Some(Instant::now());
        }
    }

    pub fn set_device(&mut self, device_id: DeviceId) {
        if self.selected_device.id == device_id {
            return;
        }
        re_log::debug!("Setting device: {:?}", device_id);
        self.backend_comms.set_device(device_id);
    }

    pub fn set_device_config(&mut self, config: &mut DeviceConfig) {
        // Don't try to set pipeline in ws not connected or device not selected
        if !self
            .backend_comms
            .ws
            .connected
            .load(std::sync::atomic::Ordering::SeqCst)
            || self.selected_device.id == ""
        {
            return;
        }
        config.left_camera.board_socket = BoardSocket::LEFT;
        config.right_camera.board_socket = BoardSocket::RIGHT;
        if !config.depth_enabled {
            config.depth = None;
        }
        self.backend_comms.set_pipeline(&config);
        re_log::info!("Creating pipeline...");
        self.applied_device_config.update_in_progress = true;
    }
}

pub type DeviceId = String;
