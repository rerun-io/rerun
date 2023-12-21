use super::ContainerKind;

impl From<egui_tiles::ContainerKind> for ContainerKind {
    fn from(value: egui_tiles::ContainerKind) -> Self {
        match value {
            egui_tiles::ContainerKind::Tabs => Self(1),
            egui_tiles::ContainerKind::Horizontal => Self(2),
            egui_tiles::ContainerKind::Vertical => Self(3),
            egui_tiles::ContainerKind::Grid => Self(4),
        }
    }
}

impl From<ContainerKind> for egui_tiles::ContainerKind {
    fn from(value: ContainerKind) -> Self {
        match value.0 {
            1 => egui_tiles::ContainerKind::Tabs,
            2 => egui_tiles::ContainerKind::Horizontal,
            3 => egui_tiles::ContainerKind::Vertical,
            4 => egui_tiles::ContainerKind::Grid,
            _ => egui_tiles::ContainerKind::default(),
        }
    }
}
