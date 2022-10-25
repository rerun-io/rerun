/// The six cardinal directions for 3D view-space and image-space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum RelativeDirection {
    Up,
    Down,
    Right,
    Left,
    Forward,
    Back,
}

impl RelativeDirection {
    pub fn short(&self) -> &'static str {
        match self {
            Self::Up => "U",
            Self::Down => "D",
            Self::Right => "R",
            Self::Left => "L",
            Self::Forward => "F",
            Self::Back => "B",
        }
    }
    pub fn long(&self) -> &'static str {
        match self {
            Self::Up => "Up",
            Self::Down => "Down",
            Self::Right => "Right",
            Self::Left => "Left",
            Self::Forward => "Forward",
            Self::Back => "Back",
        }
    }
}

/// For 3D view-space and image-space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct RelativeSystem(pub [RelativeDirection; 3]);

impl RelativeSystem {
    pub fn describe(&self) -> String {
        let [x, y, z] = self.0;
        format!(
            "{}{}{} (X={}, Y={}, Z={})",
            x.short(),
            y.short(),
            z.short(),
            x.long(),
            y.long(),
            z.long()
        )
    }
}

/// The size cardinal directions for 3D worlds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum AbsoluteDirection {
    Up,
    Down,
    East,
    West,
    North,
    South,
}

impl AbsoluteDirection {
    pub fn short(&self) -> &'static str {
        match self {
            Self::Up => "U",
            Self::Down => "D",
            Self::East => "E",
            Self::West => "W",
            Self::North => "N",
            Self::South => "S",
        }
    }
    pub fn long(&self) -> &'static str {
        match self {
            Self::Up => "Up",
            Self::Down => "Down",
            Self::East => "East",
            Self::West => "West",
            Self::North => "North",
            Self::South => "South",
        }
    }
}

/// For 3D worlds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct AbsoluteSystem(pub [AbsoluteDirection; 3]);

impl AbsoluteSystem {
    pub fn describe(&self) -> String {
        let [x, y, z] = self.0;
        format!(
            "{}{}{} (X={}, Y={}, Z={})",
            x.short(),
            y.short(),
            z.short(),
            x.long(),
            y.long(),
            z.long()
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Axis3 {
    X,
    Y,
    Z,
}

impl std::fmt::Display for Axis3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::X => "X".fmt(f),
            Self::Y => "Y".fmt(f),
            Self::Z => "Z".fmt(f),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Handedness {
    Right,
    Left,
}

impl Handedness {
    pub fn describe(&self) -> &'static str {
        match self {
            Self::Left => "left handed",
            Self::Right => "right handed",
        }
    }
}

// For 3D worlds
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum WorldSystem {
    /// For when you don't know/care about where north is.
    Partial {
        up: Option<Axis3>,
        handedness: Handedness,
    },

    /// For when you know where north is.
    Full(AbsoluteSystem),
}

impl WorldSystem {
    pub fn describe(&self) -> String {
        match self {
            Self::Partial { up, handedness } => {
                if let Some(up) = up {
                    format!("Up = {}, {}", up, handedness.describe())
                } else {
                    handedness.describe().to_owned()
                }
            }
            Self::Full(system) => system.describe(),
        }
    }
}

/// How we interpret the coordinate system of an object/space.
///
/// For instance: What is "up"? What does the Z axis mean? Is this right-handed or left-handed?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum CoordinateSystem {
    /// For 3D worlds
    World(WorldSystem),

    /// For anything else (cameras, images, …)
    Relative(RelativeSystem),
}

impl CoordinateSystem {
    pub fn describe(&self) -> String {
        match self {
            Self::World(system) => system.describe(),
            Self::Relative(system) => system.describe(),
        }
    }
}
