use arrow2::datatypes::DataType;
use arrow2_convert::{
    deserialize::ArrowDeserialize,
    field::{ArrowField, FixedSizeBinary},
    serialize::ArrowSerialize,
};

use crate::msg_bundle::Component;

/// The six cardinal directions for 3D view-space and image-space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum ViewDir {
    Up = 1,
    Down = 2,
    Right = 3,
    Left = 4,
    Forward = 5,
    Back = 6,
}

impl ViewDir {
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

impl TryFrom<u8> for ViewDir {
    type Error = super::FieldError;

    #[inline]
    fn try_from(i: u8) -> super::Result<Self> {
        match i {
            1 => Ok(Self::Up),
            2 => Ok(Self::Down),
            3 => Ok(Self::Right),
            4 => Ok(Self::Left),
            5 => Ok(Self::Forward),
            6 => Ok(Self::Back),
            _ => Err(super::FieldError::BadValue),
        }
    }
}

// ----------------------------------------------------------------------------

/// How we interpret the coordinate system of an entity/space.
///
/// For instance: What is "up"? What does the Z axis mean? Is this right-handed or left-handed?
///
/// For 3D view-space and image-space.
///
/// ```
/// use re_log_types::component_types::ViewCoordinates;
/// use arrow2_convert::field::ArrowField;
/// use arrow2::datatypes::{DataType, Field};
///
/// assert_eq!(
///     ViewCoordinates::data_type(),
///     DataType::FixedSizeBinary(3)
/// );
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ViewCoordinates(pub [ViewDir; 3]);

impl Component for ViewCoordinates {
    #[inline]
    fn name() -> crate::ComponentName {
        "rerun.view_coordinates".into()
    }
}

impl ViewCoordinates {
    /// Choses a coordinate system based on just an up-axis.
    pub fn from_up_and_handedness(up: SignedAxis3, handedness: Handedness) -> Self {
        use ViewDir::{Back, Down, Forward, Right, Up};
        match handedness {
            Handedness::Right => match up {
                SignedAxis3::POSITIVE_X => Self([Up, Right, Forward]),
                SignedAxis3::NEGATIVE_X => Self([Down, Right, Back]),
                SignedAxis3::POSITIVE_Y => Self([Right, Up, Back]),
                SignedAxis3::NEGATIVE_Y => Self([Right, Down, Forward]),
                SignedAxis3::POSITIVE_Z => Self([Right, Forward, Up]),
                SignedAxis3::NEGATIVE_Z => Self([Right, Back, Down]),
            },
            Handedness::Left => match up {
                SignedAxis3::POSITIVE_X => Self([Up, Right, Back]),
                SignedAxis3::NEGATIVE_X => Self([Down, Right, Forward]),
                SignedAxis3::POSITIVE_Y => Self([Right, Up, Forward]),
                SignedAxis3::NEGATIVE_Y => Self([Right, Down, Back]),
                SignedAxis3::POSITIVE_Z => Self([Right, Back, Up]),
                SignedAxis3::NEGATIVE_Z => Self([Right, Forward, Down]),
            },
        }
    }

    /// Returns an error if this does not span all three dimensions.
    pub fn sanity_check(&self) -> Result<(), String> {
        let mut dims = [false; 3];
        for dir in self.0 {
            let dim = match dir {
                ViewDir::Up | ViewDir::Down => 0,
                ViewDir::Right | ViewDir::Left => 1,
                ViewDir::Forward | ViewDir::Back => 2,
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
            if dir == ViewDir::Up {
                return Some(SignedAxis3::new(Sign::Positive, Axis3::from_dim(dim)));
            } else if dir == ViewDir::Down {
                return Some(SignedAxis3::new(Sign::Negative, Axis3::from_dim(dim)));
            }
        }
        None
    }

    #[inline]
    pub fn right(&self) -> Option<SignedAxis3> {
        for (dim, &dir) in self.0.iter().enumerate() {
            if dir == ViewDir::Right {
                return Some(SignedAxis3::new(Sign::Positive, Axis3::from_dim(dim)));
            } else if dir == ViewDir::Left {
                return Some(SignedAxis3::new(Sign::Negative, Axis3::from_dim(dim)));
            }
        }
        None
    }

    #[inline]
    pub fn forward(&self) -> Option<SignedAxis3> {
        for (dim, &dir) in self.0.iter().enumerate() {
            if dir == ViewDir::Forward {
                return Some(SignedAxis3::new(Sign::Positive, Axis3::from_dim(dim)));
            } else if dir == ViewDir::Back {
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

    /// Returns a matrix that translates RUB to this coordinate system.
    ///
    /// (RUB: X=Right, Y=Up, B=Back)
    #[cfg(feature = "glam")]
    #[inline]
    pub fn from_rub(&self) -> glam::Mat3 {
        self.to_rub().transpose()
    }

    /// Returns a matrix that translates this coordinate system to RUB.
    ///
    /// (RUB: X=Right, Y=Up, B=Back)
    #[cfg(feature = "glam")]
    #[inline]
    pub fn to_rub(&self) -> glam::Mat3 {
        fn rub(dir: ViewDir) -> [f32; 3] {
            match dir {
                ViewDir::Right => [1.0, 0.0, 0.0],
                ViewDir::Left => [-1.0, 0.0, 0.0],
                ViewDir::Up => [0.0, 1.0, 0.0],
                ViewDir::Down => [0.0, -1.0, 0.0],
                ViewDir::Back => [0.0, 0.0, 1.0],
                ViewDir::Forward => [0.0, 0.0, -1.0],
            }
        }

        glam::Mat3::from_cols_array_2d(&[rub(self.0[0]), rub(self.0[1]), rub(self.0[2])])
    }

    #[cfg(feature = "glam")]
    #[inline]
    pub fn handedness(&self) -> Option<Handedness> {
        let to_rub = self.to_rub();
        let det = to_rub.determinant();
        if det == -1.0 {
            Some(Handedness::Left)
        } else if det == 0.0 {
            None // bad system that doesn't pass the sanity check
        } else {
            Some(Handedness::Right)
        }
    }
}

impl std::str::FromStr for ViewCoordinates {
    type Err = String;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.as_bytes() {
            [x, y, z] => {
                let slf = Self([
                    ViewDir::from_ascii_char(*x)?,
                    ViewDir::from_ascii_char(*y)?,
                    ViewDir::from_ascii_char(*z)?,
                ]);
                slf.sanity_check()?;
                Ok(slf)
            }
            _ => Err(format!("Expected three letters, got: {s:?}")),
        }
    }
}

impl ArrowField for ViewCoordinates {
    type Type = Self;

    #[inline]
    fn data_type() -> DataType {
        <FixedSizeBinary<3> as ArrowField>::data_type()
    }
}

impl ArrowSerialize for ViewCoordinates {
    type MutableArrayType = <FixedSizeBinary<3> as ArrowSerialize>::MutableArrayType;

    #[inline]
    fn new_array() -> Self::MutableArrayType {
        FixedSizeBinary::<3>::new_array()
    }

    #[inline]
    fn arrow_serialize(v: &Self, array: &mut Self::MutableArrayType) -> arrow2::error::Result<()> {
        let bytes = [v.0[0] as u8, v.0[1] as u8, v.0[2] as u8];
        array.try_push(Some(bytes))
    }
}

impl ArrowDeserialize for ViewCoordinates {
    type ArrayType = <FixedSizeBinary<3> as ArrowDeserialize>::ArrayType;

    #[inline]
    fn arrow_deserialize(
        bytes: <&Self::ArrayType as IntoIterator>::Item,
    ) -> Option<<Self as ArrowField>::Type> {
        bytes.and_then(|bytes| {
            let dirs = [
                bytes[0].try_into().ok()?,
                bytes[1].try_into().ok()?,
                bytes[2].try_into().ok()?,
            ];
            Some(ViewCoordinates(dirs))
        })
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
            _ => panic!("Expected a 3D axis, got {dim}"),
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
    pub const POSITIVE_X: Self = Self::new(Sign::Positive, Axis3::X);
    pub const NEGATIVE_X: Self = Self::new(Sign::Negative, Axis3::X);
    pub const POSITIVE_Y: Self = Self::new(Sign::Positive, Axis3::Y);
    pub const NEGATIVE_Y: Self = Self::new(Sign::Negative, Axis3::Y);
    pub const POSITIVE_Z: Self = Self::new(Sign::Positive, Axis3::Z);
    pub const NEGATIVE_Z: Self = Self::new(Sign::Negative, Axis3::Z);

    #[inline]
    pub const fn new(sign: Sign, axis: Axis3) -> Self {
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
    pub const fn from_right_handed(right_handed: bool) -> Self {
        if right_handed {
            Handedness::Right
        } else {
            Handedness::Left
        }
    }

    #[inline]
    pub fn describe(&self) -> &'static str {
        match self {
            Self::Left => "left handed",
            Self::Right => "right handed",
        }
    }
}

// ----------------------------------------------------------------------------

#[cfg(feature = "glam")]
#[test]
fn view_coordinatess() {
    use glam::{vec3, Mat3};

    {
        assert!("UUDDLRLRBAStart".parse::<ViewCoordinates>().is_err());
        assert!("UUD".parse::<ViewCoordinates>().is_err());

        let rub = "RUB".parse::<ViewCoordinates>().unwrap();
        let bru = "BRU".parse::<ViewCoordinates>().unwrap();

        assert_eq!(rub.to_rub(), Mat3::IDENTITY);
        assert_eq!(
            bru.to_rub(),
            Mat3::from_cols_array_2d(&[[0., 0., 1.], [1., 0., 0.], [0., 1., 0.]])
        );
        assert_eq!(bru.to_rub() * vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0));
    }

    {
        let cardinal_direction = [
            SignedAxis3::POSITIVE_X,
            SignedAxis3::NEGATIVE_X,
            SignedAxis3::POSITIVE_Y,
            SignedAxis3::NEGATIVE_Y,
            SignedAxis3::POSITIVE_Z,
            SignedAxis3::NEGATIVE_Z,
        ];

        for axis in cardinal_direction {
            for handedness in [Handedness::Right, Handedness::Left] {
                let system = ViewCoordinates::from_up_and_handedness(axis, handedness);
                assert_eq!(system.handedness(), Some(handedness));

                let det = system.to_rub().determinant();
                assert!(det == -1.0 || det == 0.0 || det == 1.0);

                let short = system.describe_short();
                assert_eq!(short.parse(), Ok(system));
            }
        }
    }
}

#[test]
fn test_viewcoordinates_roundtrip() {
    use arrow2::array::Array;
    use arrow2_convert::{deserialize::TryIntoCollection, serialize::TryIntoArrow};

    let views_in = vec![
        "RUB".parse::<ViewCoordinates>().unwrap(),
        "LFD".parse::<ViewCoordinates>().unwrap(),
    ];
    let array: Box<dyn Array> = views_in.try_into_arrow().unwrap();
    let views_out: Vec<ViewCoordinates> = TryIntoCollection::try_into_collection(array).unwrap();
    assert_eq!(views_in, views_out);
}
