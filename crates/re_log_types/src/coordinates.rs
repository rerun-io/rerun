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
    fn from_ascii_char(c: u8) -> Result<Self, String> {
        match c {
            b'U' => Ok(Self::Up),
            b'D' => Ok(Self::Down),
            b'R' => Ok(Self::Right),
            b'L' => Ok(Self::Left),
            b'F' => Ok(Self::Forward),
            b'B' => Ok(Self::Back),
            _ => Err("Expected one of UDRLFB (Up Down Right Left Forward Back)".to_owned()),
        }
    }

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
    /// Returns an error if this does not span all three dimensions.
    pub fn sanity_check(&self) -> Result<(), String> {
        let mut dims = [false; 3];
        for dir in self.0 {
            let dim = match dir {
                RelativeDirection::Up | RelativeDirection::Down => 0,
                RelativeDirection::Right | RelativeDirection::Left => 1,
                RelativeDirection::Forward | RelativeDirection::Back => 2,
            };
            dims[dim] = true;
        }
        if dims == [true; 3] {
            Ok(())
        } else {
            Err(format!(
                "Coordinate system does not cover all three cardinal directions: {}",
                self.describe()
            ))
        }
    }

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

    pub fn describe_short(&self) -> String {
        let [x, y, z] = self.0;
        format!("{}{}{}", x.short(), y.short(), z.short(),)
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

    /// Returns a matrix that translated RUB to this coordinate system.
    ///
    /// (RUB: X=Right, Y=Up, B=Back)
    #[cfg(feature = "glam")]
    pub fn from_rub(&self) -> glam::Mat3 {
        self.to_rub().transpose()
    }

    /// Returns a matrix that translated this coordinate system to RUB.
    ///
    /// (RUB: X=Right, Y=Up, B=Back)
    #[cfg(feature = "glam")]
    pub fn to_rub(&self) -> glam::Mat3 {
        fn rub(dir: RelativeDirection) -> [f32; 3] {
            match dir {
                RelativeDirection::Right => [1.0, 0.0, 0.0],
                RelativeDirection::Left => [-1.0, 0.0, 0.0],
                RelativeDirection::Up => [0.0, 1.0, 0.0],
                RelativeDirection::Down => [0.0, -1.0, 0.0],
                RelativeDirection::Back => [0.0, 0.0, 1.0],
                RelativeDirection::Forward => [0.0, 0.0, -1.0],
            }
        }

        glam::Mat3::from_cols_array_2d(&[rub(self.0[0]), rub(self.0[1]), rub(self.0[2])])
    }
}

impl std::str::FromStr for RelativeSystem {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.as_bytes() {
            [x, y, z] => {
                let slf = Self([
                    RelativeDirection::from_ascii_char(*x)?,
                    RelativeDirection::from_ascii_char(*y)?,
                    RelativeDirection::from_ascii_char(*z)?,
                ]);
                slf.sanity_check()?;
                Ok(slf)
            }
            _ => Err(format!("Expected three letters, got: {s:?}")),
        }
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

impl std::str::FromStr for SignedAxis3 {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "+X" => Ok(Self::new(Sign::Positive, Axis3::X)),
            "-X" => Ok(Self::new(Sign::Negative, Axis3::X)),
            "+Y" => Ok(Self::new(Sign::Positive, Axis3::Y)),
            "-Y" => Ok(Self::new(Sign::Negative, Axis3::Y)),
            "+Z" => Ok(Self::new(Sign::Positive, Axis3::Z)),
            "-Z" => Ok(Self::new(Sign::Negative, Axis3::Z)),
            _ => Err("Expected one of: +X -X +Y -Y +Z -Z".to_owned()),
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

// ----------------------------------------------------------------------------

#[cfg(feature = "glam")]
#[test]
fn coordinate_systems() {
    use glam::*;

    assert!("UUDDLRLRBAStart".parse::<RelativeSystem>().is_err());
    assert!("UUD".parse::<RelativeSystem>().is_err());

    let rub = "RUB".parse::<RelativeSystem>().unwrap();
    let bru = "BRU".parse::<RelativeSystem>().unwrap();

    assert_eq!(rub.to_rub(), Mat3::IDENTITY);
    assert_eq!(
        bru.to_rub(),
        Mat3::from_cols_array_2d(&[[0., 0., 1.], [1., 0., 0.], [0., 1., 0.]])
    );
    assert_eq!(bru.to_rub() * vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0));
}
