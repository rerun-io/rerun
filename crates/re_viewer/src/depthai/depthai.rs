use itertools::Itertools;
use re_log_types::EntityPath;

use super::api::BackendCommChannel;
use super::ws::WsMessageData;
use instant::Instant;
use std::fmt;

use strum::EnumIter;
use strum::IntoEnumIterator;

#[derive(serde::Deserialize, serde::Serialize, Clone, PartialEq, Debug)]
pub struct CameraConfig {
    pub fps: u8,
    pub resolution: CameraSensorResolution,
    pub kind: CameraSensorKind,
    pub board_socket: CameraBoardSocket,
    pub name: String,
    pub stream_enabled: bool,
}

impl Default for CameraConfig {
    fn default() -> Self {
        Self {
            fps: 30,
            resolution: CameraSensorResolution::THE_1080_P,
            kind: CameraSensorKind::COLOR,
            board_socket: CameraBoardSocket::CAM_B,
            name: String::from("color"),
            stream_enabled: true,
        }
    }
}

impl CameraConfig {
    pub fn left() -> Self {
        Self {
            board_socket: CameraBoardSocket::CAM_B,
            ..Default::default()
        }
    }

    pub fn right() -> Self {
        Self {
            board_socket: CameraBoardSocket::CAM_C,
            ..Default::default()
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq, EnumIter, Debug)]
#[allow(non_camel_case_types)]
pub enum CameraBoardSocket {
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

impl Default for CameraBoardSocket {
    fn default() -> Self {
        Self::AUTO
    }
}

impl CameraBoardSocket {
    pub fn display_name(&self, camera_features: &[CameraFeatures]) -> String {
        if let Some(cam) = camera_features.iter().find(|cam| cam.board_socket == *self) {
            if !cam.name.is_empty() {
                return format!("{} ({self:?})", cam.name);
            }
        }
        format!("{self:?}")
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Debug)]
#[allow(non_camel_case_types)]
pub enum ImuKind {
    SIX_AXIS,
    NINE_AXIS,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Debug)]
#[allow(non_camel_case_types)]
pub enum CameraSensorResolution {
    THE_400_P,
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

// fmt::Display is used in UI while fmt::Debug is used with the depthai backend api
impl fmt::Display for CameraSensorResolution {
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
            Self::THE_400_P => write!(f, "400p"),
            Self::THE_720_P => write!(f, "720p"),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq, Debug)]
#[allow(non_camel_case_types)]
pub enum CameraSensorKind {
    COLOR,
    MONO,
    TOF,
    THERMAL,
}

impl Default for CameraSensorKind {
    fn default() -> Self {
        Self::COLOR
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
pub struct CameraFeatures {
    pub resolutions: Vec<CameraSensorResolution>,
    pub max_fps: u8,
    pub board_socket: CameraBoardSocket,
    pub supported_types: Vec<CameraSensorKind>,
    pub stereo_pairs: Vec<CameraBoardSocket>, // Which cameras can be paired with this one
    pub name: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug, Default)]
pub struct DeviceProperties {
    pub id: String,
    pub cameras: Vec<CameraFeatures>,
    pub imu: Option<ImuKind>,
    pub stereo_pairs: Vec<(CameraBoardSocket, CameraBoardSocket)>,
}

impl DeviceProperties {
    pub fn has_stereo_pairs(&self) -> bool {
        !self.stereo_pairs.is_empty()
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
    pub align: CameraBoardSocket,
    pub stereo_pair: (CameraBoardSocket, CameraBoardSocket),
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
            align: CameraBoardSocket::RGB,
            stereo_pair: (CameraBoardSocket::CAM_A, CameraBoardSocket::CAM_C),
        }
    }
}

impl DepthConfig {
    pub fn from_camera_features(camera_features: &[CameraFeatures]) -> Option<Self> {
        let mut config = Self::default();
        let Some(cam_with_stereo_pair) = camera_features.iter().find(|feat| !feat.stereo_pairs.is_empty()) else {
            return None
        };
        let stereo_pair = cam_with_stereo_pair.stereo_pairs[0];
        config.stereo_pair = (cam_with_stereo_pair.board_socket, stereo_pair);
        config.align = cam_with_stereo_pair.board_socket;
        Some(config)
    }

    pub fn default_as_option() -> Option<Self> {
        Some(Self::default())
    }

    pub fn only_runtime_configs_differ(&self, other: &DepthConfig) -> bool {
        self.align == other.align && self != other
    }
}

#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub struct DeviceConfig {
    pub cameras: Vec<CameraConfig>,
    #[serde(default = "bool_true")]
    pub depth_enabled: bool, // Much easier to have an explicit bool for checkbox
    #[serde(default = "DepthConfig::default_as_option")]
    pub depth: Option<DepthConfig>,
    pub ai_model: AiModel,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            cameras: Vec::new(),
            depth_enabled: true,
            depth: Some(DepthConfig::default()),
            ai_model: AiModel::default(),
        }
    }
}

impl DeviceConfig {
    pub fn from_camera_features(camera_features: &Vec<CameraFeatures>) -> DeviceConfig {
        let mut config = DeviceConfig::default();
        for features in camera_features {
            config.cameras.push(CameraConfig {
                name: features.name.clone(),
                fps: 30, // TODO(filip): Do performance improvements to allow higher fps
                resolution: *features
                    .resolutions
                    .iter()
                    .filter(|res| {
                        res != &&CameraSensorResolution::THE_4_K
                            && res != &&CameraSensorResolution::THE_12_MP
                    })
                    .last()
                    .unwrap_or(&CameraSensorResolution::THE_800_P),
                board_socket: features.board_socket,
                stream_enabled: true,
                kind: if features.supported_types.contains(&CameraSensorKind::COLOR) {
                    CameraSensorKind::COLOR
                } else {
                    CameraSensorKind::MONO
                },
            });
        }
        config.depth = DepthConfig::from_camera_features(camera_features);
        config.ai_model = AiModel::from_camera_features(camera_features);
        config
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[allow(non_camel_case_types)]
pub enum CameraSensorType {
    COLOR,
    MONO,
    THERMAL,
    TOF,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CameraSensorConfig {
    height: i64,
    #[serde(rename = "maxFps")]
    max_fps: i64,
    #[serde(rename = "minFps")]
    min_fps: i64,
    #[serde(rename = "type")]
    kind: CameraSensorType,
    width: i64,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[allow(non_camel_case_types)]
pub enum CameraImageOrientation {
    AUTO,
    HORIZONTAL_MIRROR,
    NORMAL,
    ROTATE_180_DEG,
    VERTICAL_FLIP,
}

impl PartialEq for DeviceConfig {
    fn eq(&self, other: &Self) -> bool {
        let depth_eq = match (&self.depth, &other.depth) {
            (Some(a), Some(b)) => a == b,
            _ => true, // If one is None, it's only different if depth_enabled is different
        };
        self.cameras == other.cameras
            && depth_eq
            && self.depth_enabled == other.depth_enabled
            && self.ai_model == other.ai_model
    }
}

#[inline]
const fn bool_true() -> bool {
    true
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct DeviceConfigState {
    #[serde(skip)]
    pub config: Option<DeviceConfig>,
    #[serde(skip)]
    pub update_in_progress: bool,
}

impl fmt::Debug for DeviceConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Device config: cams: {:?}, depth: {:?}, ai_model: {:?}, depth_enabled: {:?}",
            self.cameras, self.depth, self.ai_model, self.depth_enabled
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
            message: String::from("Pipeline not started"),
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

#[derive(serde::Deserialize, serde::Serialize, Clone, fmt::Debug)]
pub struct AiModel {
    pub path: String,
    pub display_name: String,
    pub camera: CameraBoardSocket,
}

impl Default for AiModel {
    fn default() -> Self {
        default_neural_networks()[1].clone()
    }
}

impl AiModel {
    pub fn none() -> Self {
        Self {
            path: String::new(),
            display_name: String::from("No model selected"),
            camera: CameraBoardSocket::CAM_B,
        }
    }

    pub fn from_camera_features(camera_features: &[CameraFeatures]) -> Self {
        let mut model = Self::default();
        if let Some(cam) = camera_features.iter().find(|cam| cam.name == "color") {
            model.camera = cam.board_socket;
        }
        model
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
    pub selected_device: DeviceProperties,
    #[serde(skip)]
    pub applied_device_config: DeviceConfigState,
    #[serde(skip)]
    pub modified_device_config: DeviceConfig,
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
    #[serde(skip)]
    update_timeout_timer: Option<Instant>,
}

#[inline]
fn all_subscriptions() -> Vec<ChannelId> {
    ChannelId::iter().collect_vec()
}

#[inline]
fn default_neural_networks() -> Vec<AiModel> {
    vec![
        AiModel::none(),
        AiModel {
            path: String::from("yolov8n_coco_640x352"),
            display_name: String::from("Yolo V8"),
            camera: CameraBoardSocket::CAM_B,
        },
        AiModel {
            path: String::from("mobilenet-ssd"),
            display_name: String::from("MobileNet SSD"),
            camera: CameraBoardSocket::CAM_B,
        },
        AiModel {
            path: String::from("face-detection-retail-0004"),
            display_name: String::from("Face Detection"),
            camera: CameraBoardSocket::CAM_B,
        },
        AiModel {
            path: String::from("age-gender-recognition-retail-0013"),
            display_name: String::from("Age gender recognition"),
            camera: CameraBoardSocket::CAM_B,
        },
    ]
}

impl Default for State {
    fn default() -> Self {
        Self {
            devices_available: None,
            selected_device: DeviceProperties::default(),
            applied_device_config: DeviceConfigState::default(),
            modified_device_config: DeviceConfig::default(),
            subscriptions: ChannelId::iter().collect(),
            setting_subscriptions: false,
            backend_comms: BackendCommChannel::default(),
            poll_instant: Some(Instant::now()), // No default for Instant
            neural_networks: default_neural_networks(),
            update_timeout_timer: None,
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
        pub static ref DEVICE_TRANSFORM: EntityPath = EntityPath::from("camera");

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
    pub fn only_runtime_configs_changed(
        old_config: &DeviceConfig,
        new_config: &DeviceConfig,
    ) -> bool {
        let any_runtime_conf_changed = old_config.depth.is_some()
            && new_config.depth.is_some()
            && old_config
                .depth
                .unwrap()
                .only_runtime_configs_differ(&new_config.depth.unwrap()); // || others to be added
        any_runtime_conf_changed
            && old_config.cameras == new_config.cameras
            && old_config.ai_model == new_config.ai_model
    }

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

    /// Returns available devices
    pub fn get_devices(&mut self) -> Vec<DeviceId> {
        // Return stored available devices or fetch them from the api (they get fetched every 30s via poller)
        if let Some(devices) = self.devices_available.clone() {
            return devices;
        }
        Vec::new()
    }

    /// Returns cameras connected to the selected device
    pub fn get_connected_cameras(&self) -> &Vec<CameraFeatures> {
        &self.selected_device.cameras
    }

    pub fn shutdown(&mut self) {
        self.backend_comms.shutdown();
    }

    fn set_update_in_progress(&mut self, in_progress: bool) {
        self.update_timeout_timer = None;
        if in_progress {
            self.update_timeout_timer = Some(Instant::now());
        }
        self.applied_device_config.update_in_progress = in_progress;
    }

    pub fn update(&mut self) {
        if let Some(update_timeout) = self.update_timeout_timer {
            if update_timeout.elapsed().as_secs() > 30 {
                self.set_update_in_progress(false);
            }
        }

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
                WsMessageData::Pipeline((config, _)) => {
                    let mut subs = self.subscriptions.clone();
                    if config.depth.is_some() {
                        subs.push(ChannelId::DepthImage);
                    }
                    if let Some(color_camera) =
                        &config.cameras.iter().find(|cam| cam.name == "color")
                    {
                        if color_camera.stream_enabled {
                            subs.push(ChannelId::ColorImage);
                        }
                    }
                    if let Some(left_cam) = &config.cameras.iter().find(|cam| cam.name == "left") {
                        if left_cam.stream_enabled {
                            subs.push(ChannelId::LeftMono);
                        }
                    }
                    if let Some(right_cam) = &config.cameras.iter().find(|cam| cam.name == "right")
                    {
                        if right_cam.stream_enabled {
                            subs.push(ChannelId::RightMono);
                        }
                    }
                    self.applied_device_config.config = Some(config.clone());
                    self.modified_device_config = config.clone();
                    let Some(applied_device_config) = self.applied_device_config.config.as_mut() else {
                        self.reset();
                    self.applied_device_config.update_in_progress = false;
                    return;
                    };
                    applied_device_config.depth_enabled = config.depth.is_some();
                    self.modified_device_config.depth_enabled =
                        self.modified_device_config.depth.is_some();
                    self.set_subscriptions(&subs);
                    self.set_update_in_progress(false);
                }
                WsMessageData::DeviceProperties(device) => {
                    re_log::debug!("Setting device: {device:?}");
                    self.selected_device = device;
                    self.backend_comms.set_subscriptions(&self.subscriptions);
                    self.modified_device_config =
                        DeviceConfig::from_camera_features(&self.selected_device.cameras);
                    // self.backend_comms
                    //     .set_pipeline(&self.applied_device_config.config, false);
                    self.set_update_in_progress(false);
                }
                WsMessageData::Error(error) => {
                    re_log::error!("Error: {:}", error.message);
                    self.set_update_in_progress(false);
                    match error.action {
                        ErrorAction::None => (),
                        ErrorAction::FullReset => {
                            self.set_device(String::new());
                        }
                    }
                }
            }
        }

        if let Some(poll_instant) = self.poll_instant {
            if poll_instant.elapsed().as_secs() < 2 {
                return;
            }
            if self.selected_device.id.is_empty() {
                self.backend_comms.get_devices();
            }
            self.poll_instant = Some(Instant::now());
        } else {
            self.poll_instant = Some(Instant::now());
        }
    }

    pub fn set_device(&mut self, device_id: DeviceId) {
        re_log::debug!("Setting device: {:?}", device_id);
        self.applied_device_config.config = None;
        self.backend_comms.set_device(device_id);
        self.set_update_in_progress(true);
    }

    pub fn set_device_config(&mut self, config: &mut DeviceConfig, runtime_only: bool) {
        // Don't try to set pipeline in ws not connected
        if !self
            .backend_comms
            .ws
            .connected
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return;
        }
        if !config.depth_enabled {
            config.depth = None;
        }

        if !self.selected_device.has_stereo_pairs() {
            config.depth = None;
        }

        if self.selected_device.id.is_empty() {
            self.applied_device_config.config = Some(config.clone());
            return;
        }
        self.backend_comms.set_pipeline(config, runtime_only);
        if runtime_only {
            self.applied_device_config.config = Some(config.clone());
            self.set_update_in_progress(false);
        } else {
            self.set_update_in_progress(true);
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn is_update_in_progress(&self) -> bool {
        self.applied_device_config.update_in_progress
    }
}

pub type DeviceId = String;
