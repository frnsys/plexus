pub trait FromGeometry<T> {
    fn from_geometry(other: T) -> Self;
}

pub trait IntoGeometry<T> {
    fn into_geometry(self) -> T;
}

impl<T, U> IntoGeometry<U> for T
where
    U: FromGeometry<T>,
{
    fn into_geometry(self) -> U {
        U::from_geometry(self)
    }
}

impl<T> FromGeometry<T> for T {
    fn from_geometry(other: T) -> Self {
        other
    }
}

// TODO: The interior versions of geometry conversion traits do not have a
//       reflexive implementation. This allows for conversions from `Mesh<T>`
//       to `Mesh<U>`, where `T` and `U` may be the same.
//
//       This is a bit confusing; consider removing these if they aren't
//       useful.
pub trait FromInteriorGeometry<T> {
    fn from_interior_geometry(other: T) -> Self;
}

pub trait IntoInteriorGeometry<T> {
    fn into_interior_geometry(self) -> T;
}

impl<T, U> IntoInteriorGeometry<U> for T
where
    U: FromInteriorGeometry<T>,
{
    fn into_interior_geometry(self) -> U {
        U::from_interior_geometry(self)
    }
}

pub trait AsPosition {
    type Target;

    fn as_position(&self) -> &Self::Target;
    fn as_position_mut(&mut self) -> &mut Self::Target;
}

#[cfg(feature = "geometry-cgmath")]
mod feature_geometry_cgmath {}

#[cfg(feature = "geometry-nalgebra")]
mod feature_geometry_nalgebra {
    use nalgebra::{Point2, Point3, Scalar, Vector2, Vector3};
    use num::Float;

    use geometry::{Duplet, Triplet};
    use geometry::convert::*;
    use ordered::NotNan;

    impl<T> FromGeometry<Duplet<T>> for Point2<T>
    where
        T: Scalar,
    {
        fn from_geometry(other: Duplet<T>) -> Self {
            Point2::new(other.0, other.1)
        }
    }

    impl<T> FromGeometry<Triplet<T>> for Point3<T>
    where
        T: Scalar,
    {
        fn from_geometry(other: Triplet<T>) -> Self {
            Point3::new(other.0, other.1, other.2)
        }
    }

    impl<T> FromGeometry<Triplet<NotNan<T>>> for Point3<T>
    where
        T: Float + Scalar,
    {
        fn from_geometry(other: Triplet<NotNan<T>>) -> Self {
            Point3::new(
                other.0.into_inner(),
                other.1.into_inner(),
                other.2.into_inner(),
            )
        }
    }

    impl<T> FromGeometry<Point3<NotNan<T>>> for Point3<T>
    where
        T: Float + Scalar,
    {
        fn from_geometry(other: Point3<NotNan<T>>) -> Self {
            Point3::new(
                other.x.into_inner(),
                other.y.into_inner(),
                other.z.into_inner(),
            )
        }
    }

    impl<T> FromGeometry<Duplet<T>> for Vector2<T>
    where
        T: Scalar,
    {
        fn from_geometry(other: Duplet<T>) -> Self {
            Vector2::new(other.0, other.1)
        }
    }

    impl<T> FromGeometry<Triplet<T>> for Vector3<T>
    where
        T: Scalar,
    {
        fn from_geometry(other: Triplet<T>) -> Self {
            Vector3::new(other.0, other.1, other.2)
        }
    }

    impl<T> AsPosition for Point3<T>
    where
        T: Scalar,
    {
        type Target = Self;

        fn as_position(&self) -> &Self::Target {
            self
        }

        fn as_position_mut(&mut self) -> &mut Self::Target {
            self
        }
    }
}
