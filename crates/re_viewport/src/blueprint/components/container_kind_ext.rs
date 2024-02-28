use super::ContainerKind;

impl From<egui_tiles::ContainerKind> for ContainerKind {
    fn from(value: egui_tiles::ContainerKind) -> Self {
        match value {
            egui_tiles::ContainerKind::Tabs => Self::Tabs,
            egui_tiles::ContainerKind::Horizontal => Self::Horizontal,
            egui_tiles::ContainerKind::Vertical => Self::Vertical,
            egui_tiles::ContainerKind::Grid => Self::Grid,
        }
    }
}

impl From<ContainerKind> for egui_tiles::ContainerKind {
    fn from(value: ContainerKind) -> Self {
        match value {
            ContainerKind::Tabs => Self::Tabs,
            ContainerKind::Horizontal => Self::Horizontal,
            ContainerKind::Vertical => Self::Vertical,
            ContainerKind::Grid => Self::Grid,
        }
    }
}
