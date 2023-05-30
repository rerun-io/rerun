//! Conversion from and to [`crate::Scene`] for tuples of [`crate::SceneElement`]s.

use std::any::Any;

use crate::{Scene, SceneElement};

#[derive(Debug, thiserror::Error)]
pub enum SceneElementListConversionError {
    #[error("Scene element list has not the expected number of elements.")]
    ElementCountMismatch,

    #[error("Failed to downcast scene element.")]
    DowncastFailure,
}

impl TryFrom<Scene> for () {
    type Error = SceneElementListConversionError;

    fn try_from(scene: Scene) -> Result<Self, Self::Error> {
        if !scene.0.is_empty() {
            Err(SceneElementListConversionError::ElementCountMismatch)
        } else {
            Ok(())
        }
    }
}

impl From<()> for Scene {
    fn from(_val: ()) -> Self {
        Scene(Vec::new())
    }
}

impl<T0: SceneElement> TryFrom<Scene> for (T0,) {
    type Error = SceneElementListConversionError;

    fn try_from(scene: Scene) -> Result<Self, Self::Error> {
        if scene.0.len() != 1 {
            return Err(SceneElementListConversionError::ElementCountMismatch);
        }
        let mut scene_iter = scene.0.into_iter();

        let element0 = scene_iter.next().unwrap();
        let t0 = *Box::<dyn Any>::downcast::<T0>(element0.into_any())
            .map_err(|_err| SceneElementListConversionError::DowncastFailure)?;

        Ok((t0,))
    }
}

impl<T0: SceneElement> From<(T0,)> for Scene {
    fn from(val: (T0,)) -> Self {
        Scene(vec![Box::new(val.0)])
    }
}

impl<T0: SceneElement, T1: SceneElement> TryFrom<Scene> for (T0, T1) {
    type Error = SceneElementListConversionError;

    fn try_from(scene: Scene) -> Result<Self, Self::Error> {
        if scene.0.len() != 2 {
            return Err(SceneElementListConversionError::ElementCountMismatch);
        }
        let mut scene_iter = scene.0.into_iter();

        let element0 = scene_iter.next().unwrap();
        let t0 = *Box::<dyn Any>::downcast::<T0>(element0.into_any())
            .map_err(|_err| SceneElementListConversionError::DowncastFailure)?;

        let element1 = scene_iter.next().unwrap();
        let t1 = *Box::<dyn Any>::downcast::<T1>(element1.into_any())
            .map_err(|_err| SceneElementListConversionError::DowncastFailure)?;

        Ok((t0, t1))
    }
}

impl<T0: SceneElement, T1: SceneElement> From<(T0, T1)> for Scene {
    fn from(val: (T0, T1)) -> Self {
        Scene(vec![Box::new(val.0), Box::new(val.1)])
    }
}

// TODO(andreas): Devise macro to generate this for tuples of arbitrary length.
