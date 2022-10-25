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
    #[inline]
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

    #[inline]
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

// ----------------------------------------------------------------------------

/// For 3D view-space and image-space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct RelativeSystem(pub [RelativeDirection; 3]);

impl RelativeSystem {
    #[inline]
    pub fn up(&self) -> Option<SignedAxis3> {
        for (dim, &dir) in self.0.iter().enumerate() {
            if dir == RelativeDirection::Up {
                return Some(SignedAxis3::new(Sign::Positive, Axis3::from_dim(dim)));
            } else if dir == RelativeDirection::Down {
                return Some(SignedAxis3::new(Sign::Negative, Axis3::from_dim(dim)));
            }
        }
        None
    }

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

// ----------------------------------------------------------------------------

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
    #[inline]
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

    #[inline]
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

// ----------------------------------------------------------------------------

/// For 3D worlds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct AbsoluteSystem(pub [AbsoluteDirection; 3]);

impl AbsoluteSystem {
    pub fn up(&self) -> Option<SignedAxis3> {
        for (dim, &dir) in self.0.iter().enumerate() {
            if dir == AbsoluteDirection::Up {
                return Some(SignedAxis3::new(Sign::Positive, Axis3::from_dim(dim)));
            } else if dir == AbsoluteDirection::Down {
                return Some(SignedAxis3::new(Sign::Negative, Axis3::from_dim(dim)));
            }
        }
        None
    }

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

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Axis3 {
    X,
    Y,
    Z,
}

impl Axis3 {
    #[inline]
    pub fn from_dim(dim: usize) -> Self {
        match dim {
            0 => Self::X,
            1 => Self::Y,
            2 => Self::Z,
            _ => panic!("Only 3D"),
        }
    }
}

impl std::fmt::Display for Axis3 {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::X => "X".fmt(f),
            Self::Y => "Y".fmt(f),
            Self::Z => "Z".fmt(f),
        }
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Sign {
    Positive,
    Negative,
}

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct SignedAxis3 {
    pub sign: Sign,
    pub axis: Axis3,
}

impl SignedAxis3 {
    #[inline]
    pub fn new(sign: Sign, axis: Axis3) -> Self {
        Self { sign, axis }
    }

    #[inline]
    pub fn as_vec3(&self) -> [f32; 3] {
        match (self.sign, self.axis) {
            (Sign::Positive, Axis3::X) => [1.0, 0.0, 0.0],
            (Sign::Negative, Axis3::X) => [-1.0, 0.0, 0.0],
            (Sign::Positive, Axis3::Y) => [0.0, 1.0, 0.0],
            (Sign::Negative, Axis3::Y) => [0.0, -1.0, 0.0],
            (Sign::Positive, Axis3::Z) => [0.0, 0.0, 1.0],
            (Sign::Negative, Axis3::Z) => [0.0, 0.0, -1.0],
        }
    }
}

impl std::fmt::Display for SignedAxis3 {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sign = match self.sign {
            Sign::Positive => "+",
            Sign::Negative => "-",
        };
        write!(f, "{}{}", sign, self.axis)
    }
}

pub struct SignedAxis3Error;

impl std::fmt::Display for SignedAxis3Error {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        "Expected one of: +X -X +Y -Y +Z -Z".fmt(f)
    }
}

impl std::str::FromStr for SignedAxis3 {
    type Err = SignedAxis3Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "+X" => Ok(Self::new(Sign::Positive, Axis3::X)),
            "-X" => Ok(Self::new(Sign::Negative, Axis3::X)),
            "+Y" => Ok(Self::new(Sign::Positive, Axis3::Y)),
            "-Y" => Ok(Self::new(Sign::Negative, Axis3::Y)),
            "+Z" => Ok(Self::new(Sign::Positive, Axis3::Z)),
            "-Z" => Ok(Self::new(Sign::Negative, Axis3::Z)),
            _ => Err(SignedAxis3Error),
        }
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Handedness {
    Right,
    Left,
}

impl Handedness {
    #[inline]
    pub fn describe(&self) -> &'static str {
        match self {
            Self::Left => "left handed",
            Self::Right => "right handed",
        }
    }
}

// ----------------------------------------------------------------------------

// For 3D worlds
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum WorldSystem {
    /// For when you don't know/care about where north is.
    Partial {
        up: Option<SignedAxis3>,
        handedness: Handedness,
    },

    /// For when you know where north is.
    Full(AbsoluteSystem),
}

impl WorldSystem {
    #[inline]
    pub fn up(&self) -> Option<SignedAxis3> {
        match self {
            Self::Partial { up, .. } => *up,
            Self::Full(system) => system.up(),
        }
    }

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

// ----------------------------------------------------------------------------

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
    /// What axis is the up-axis?
    #[inline]
    pub fn up(&self) -> Option<SignedAxis3> {
        match self {
            Self::World(system) => system.up(),
            Self::Relative(system) => system.up(),
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Self::World(system) => system.describe(),
            Self::Relative(system) => system.describe(),
        }
    }
}
