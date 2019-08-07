//! Half-edge graph representation of meshes.
//!
//! This module provides a flexible representation of meshes as a [half-edge
//! graph](https://en.wikipedia.org/wiki/doubly_connected_edge_list).
//! _Half-edges_ and _edges_ are referred to as _arcs_ and _edges_,
//! respectively. Meshes can store arbitrary geometric data associated with
//! any topological structure (vertices, arcs, edges, and faces).
//!
//! Geometry is vertex-based, meaning that geometric operations depend on
//! vertices exposing some notion of positional data. See the `geometry` module
//! and `AsPosition` trait. If geometry does not have this property, then
//! spatial operations will not be available.
//!
//! See the [user guide](https://plexus.rs/user-guide/graphs) for more details
//! and examples.
//!
//! # Representation
//!
//! A `MeshGraph` is conceptually composed of _vertices_, _arcs_, _edges_, and
//! _faces_. The figure below summarizes the connectivity in a `MeshGraph`.
//!
//! ![Half-Edge Graph Figure](https://plexus.rs/img/heg.svg)
//!
//! Arcs are directed and connect vertices. An arc that is directed toward a
//! vertex $A$ is an _incoming arc_ with respect to $A$.  Similarly, an arc
//! directed away from such a vertex is an _outgoing arc_. Every vertex is
//! associated with exactly one _leading arc_, which is always an outgoing arc.
//! The vertex toward which an arc is directed is the arc's _destination
//! vertex_ and the other is its _source vertex_.
//!
//! Every arc is paired with an _opposite arc_ with an opposing direction.
//! Given an arc from a vertex $A$ to a vertex $B$, that arc will have an
//! opposite arc from $B$ to $A$. Such arcs are notated $\overrightarrow{AB}$
//! and $\overrightarrow{BA}$. Together, these arcs form an _edge_, which is
//! not directed. An edge and its two arcs are together called a _composite
//! edge_.
//!
//! Arcs are connected to their neighbors, known as _next_ and _previous arcs_.
//! A traversal along a series of arcs is a _path_. The path formed by
//! traversing from an arc to its next arc and so on is a _ring_.  When a face
//! is present within an ring, the arcs will refer to that face and the face
//! will refer to exactly one of the arcs in the ring (this is the leading arc
//! of the face). An arc with no associated face is known as a _boundary arc_.
//! If both of an edge's arcs are boundary arcs, then that edge is an
//! _unbounded edge_.
//!
//! A path that terminates is _open_ and a path that forms a loop is _closed_.
//! Rings are always closed. A path over vertices $A$, $B$, and $C$ is
//! notated $\overrightarrow{\\{A, B, C\\}}$.
//!
//! Together with vertices and faces, the connectivity of arcs allows for
//! effecient traversals of topology. For example, it becomes trivial to find
//! neighboring topologies, such as the faces that share a given vertex or the
//! neighboring faces of a given face.
//!
//! `MeshGraph`s store topological data using associative collections and mesh
//! data is accessed using keys into this storage. Keys are exposed as strongly
//! typed and opaque values, which can be used to refer to a topological
//! structure.
//!
//! # Topological Views
//!
//! `MeshGraph`s expose _views_ over their topological structures (vertices,
//! arcs, edges, and faces). Views are accessed via keys or iteration and
//! behave similarly to references. They provide the primary API for
//! interacting with a `MeshGraph`'s topology and geometry. There are three
//! types summarized below:
//!
//! | Type      | Traversal | Exclusive | Geometry  | Topology  |
//! |-----------|-----------|-----------|-----------|-----------|
//! | Immutable | Yes       | No        | Immutable | Immutable |
//! | Mutable   | Yes       | Yes       | Mutable   | Mutable   |
//! | Orphan    | No        | No        | Mutable   | N/A       |
//!
//! _Immutable_ and _mutable views_ behave similarly to references: immutable
//! views cannot mutate a graph and are not exclusive while mutable views may
//! mutate both the geometry and topology of a graph but are exclusive.
//!
//! _Orphan views_ are similar to mutable views in that they may mutate the
//! geometry of a graph, but they do not have access to the topology of a
//! graph. Because they do not know about other vertices, arcs, etc., an orphan
//! view cannot traverse a graph in any way. These views are most useful for
//! modifying the geometry of a graph and, unlike mutable views, they are not
//! exclusive. Iterators over topological structures in a graph sometimes emit
//! orphan views.
//!
//! # Examples
//!
//! Generating a mesh from a $uv$-sphere:
//!
//! ```rust
//! # extern crate decorum;
//! # extern crate nalgebra;
//! # extern crate plexus;
//! #
//! use decorum::N64;
//! use nalgebra::Point3;
//! use plexus::graph::MeshGraph;
//! use plexus::prelude::*;
//! use plexus::primitive::generate::Position;
//! use plexus::primitive::sphere::UvSphere;
//!
//! # fn main() {
//! let mut graph = UvSphere::default()
//!     .polygons::<Position<Point3<N64>>>()
//!     .collect::<MeshGraph<Point3<N64>>>();
//! # }
//! ```
//!
//! Extruding a face in a mesh:
//!
//! ```rust
//! # extern crate decorum;
//! # extern crate nalgebra;
//! # extern crate plexus;
//! #
//! use decorum::N64;
//! use nalgebra::Point3;
//! use plexus::graph::MeshGraph;
//! use plexus::prelude::*;
//! use plexus::primitive::generate::Position;
//! use plexus::primitive::sphere::UvSphere;
//!
//! # fn main() {
//! let mut graph = UvSphere::new(8, 8)
//!     .polygons::<Position<Point3<N64>>>()
//!     .collect::<MeshGraph<Point3<N64>>>();
//! let key = graph.faces().nth(0).unwrap().key(); // Get the key of the first face.
//! let face = graph.face_mut(key).unwrap().extrude(1.0); // Extrude the face.
//! # }
//! ```
//!
//! Traversing and circulating over a mesh:
//!
//! ```rust
//! # extern crate nalgebra;
//! # extern crate plexus;
//! #
//! use nalgebra::Point2;
//! use plexus::graph::MeshGraph;
//! use plexus::prelude::*;
//! use plexus::primitive::Tetragon;
//!
//! # fn main() {
//! let mut graph = MeshGraph::<Point2<f64>>::from_raw_buffers(
//!     vec![Tetragon::new(0u32, 1, 2, 3)],
//!     vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
//! )
//! .unwrap();
//! graph.triangulate();
//!
//! // Traverse an arc and use a circulator to get the faces of a nearby vertex.
//! let key = graph.arcs().nth(0).unwrap().key();
//! let mut vertex = graph
//!     .arc_mut(key)
//!     .unwrap()
//!     .into_opposite_arc()
//!     .into_next_arc()
//!     .into_destination_vertex();
//! for mut face in vertex.neighboring_orphan_faces() {
//!     // `face.geometry` is mutable here.
//! }
//! # }
//! ```

mod borrow;
mod core;
mod geometry;
mod mutation;
mod storage;
mod view;

use decorum::N64;
use failure::Fail;
use itertools::{Itertools, MinMaxResult};
use num::{Integer, NumCast, ToPrimitive, Unsigned};
use smallvec::SmallVec;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::Debug;
use std::hash::Hash;
use std::iter::FromIterator;
use theon::ops::Map;
use theon::query::Aabb;
use theon::space::{EuclideanSpace, Scalar};
use theon::AsPosition;
use typenum::{self, NonZero};

use crate::buffer::{BufferError, MeshBuffer};
use crate::encoding::{FaceDecoder, FromEncoding, VertexDecoder};
use crate::graph::core::{Bind, Core, OwnedCore};
use crate::graph::mutation::{Consistent, Mutate, Mutation};
use crate::graph::storage::alias::*;
use crate::graph::storage::key::OpaqueKey;
use crate::graph::storage::{AsStorage, AsStorageMut, StorageProxy};
use crate::graph::view::{IntoView, OrphanView};
use crate::index::{Flat, FromIndexer, Grouping, HashIndexer, IndexBuffer, IndexVertices, Indexer};
use crate::primitive::decompose::IntoVertices;
use crate::primitive::Polygonal;
use crate::{Arity, FromRawBuffers, FromRawBuffersWithArity, IntoGeometry};

pub use crate::graph::geometry::{
    ArcNormal, EdgeMidpoint, FaceCentroid, FaceNormal, FacePlane, GraphGeometry, VertexCentroid,
    VertexNormal, VertexPosition,
};
pub use crate::graph::storage::key::{ArcKey, EdgeKey, FaceKey, VertexKey};
pub use crate::graph::storage::payload::{ArcPayload, EdgePayload, FacePayload, VertexPayload};
pub use crate::graph::view::edge::{ArcView, EdgeView, OrphanArcView, OrphanEdgeView};
pub use crate::graph::view::face::{FaceView, OrphanFaceView, RingView};
pub use crate::graph::view::vertex::{OrphanVertexView, VertexView};

pub use Selector::ByIndex;
pub use Selector::ByKey;

#[derive(Debug, Fail, PartialEq)]
pub enum GraphError {
    #[fail(display = "required topology not found")]
    TopologyNotFound,
    #[fail(display = "conflicting topology found")]
    TopologyConflict,
    #[fail(display = "topology malformed")]
    TopologyMalformed,
    #[fail(display = "arity is non-polygonal")]
    ArityNonPolygonal,
    #[fail(
        display = "conflicting arity; expected {}, but got {}",
        expected, actual
    )]
    ArityConflict { expected: usize, actual: usize },
    #[fail(display = "arity is non-uniform")]
    ArityNonUniform,
    #[fail(display = "geometric operation failed")]
    Geometry,
}

impl From<BufferError> for GraphError {
    fn from(_: BufferError) -> Self {
        // TODO: How should buffer errors be handled? Is this sufficient?
        GraphError::TopologyMalformed
    }
}

trait OptionExt<T> {
    fn expect_consistent(self) -> T;
}

impl<T> OptionExt<T> for Option<T> {
    fn expect_consistent(self) -> T {
        self.expect("internal error: graph consistency violated")
    }
}

trait ResultExt<T, E> {
    fn expect_consistent(self) -> T
    where
        E: Debug;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn expect_consistent(self) -> T
    where
        E: Debug,
    {
        self.expect("internal error: graph consistency violated")
    }
}

/// Topology selector.
///
/// Identifies topology by key or index. Keys behave as an absolute selector
/// and uniquely identify a single topological structure. Indices behave as a
/// relative selector and identify topological structures relative to some
/// other structure. `Selector` is used by operations that support both of
/// these selection mechanisms.
///
/// An index is typically used to select a neighbor or contained (and ordered)
/// topological structure, such as a neighboring face.
///
/// # Examples
///
/// Splitting a face by index (of its contained vertices):
///
/// ```rust
/// # extern crate decorum;
/// # extern crate nalgebra;
/// # extern crate plexus;
/// #
/// use decorum::N64;
/// use nalgebra::Point3;
/// use plexus::graph::MeshGraph;
/// use plexus::prelude::*;
/// use plexus::primitive::cube::Cube;
/// use plexus::primitive::generate::Position;
///
/// # fn main() {
/// let mut graph = Cube::new()
///     .polygons::<Position<Point3<N64>>>()
///     .collect::<MeshGraph<Point3<N64>>>();
/// let key = graph.faces().nth(0).unwrap().key();
/// graph
///     .face_mut(key)
///     .unwrap()
///     .split(ByIndex(0), ByIndex(2))
///     .unwrap();
/// # }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Selector<K> {
    ByKey(K),
    ByIndex(usize),
}

impl<K> Selector<K> {
    /// Gets the selector's key or passes its index to a function to resolve
    /// the key.
    pub fn key_or_else<E, F>(self, f: F) -> Result<K, GraphError>
    where
        E: Into<GraphError>,
        F: Fn(usize) -> Result<K, E>,
    {
        match self {
            Selector::ByKey(key) => Ok(key),
            Selector::ByIndex(index) => f(index).map_err(|error| error.into()),
        }
    }

    /// Gets the selector's index or passes its key to a function to resolve
    /// the index.
    pub fn index_or_else<E, F>(self, f: F) -> Result<usize, GraphError>
    where
        E: Into<GraphError>,
        F: Fn(K) -> Result<usize, E>,
    {
        match self {
            Selector::ByKey(key) => f(key).map_err(|error| error.into()),
            Selector::ByIndex(index) => Ok(index),
        }
    }
}

impl<K> From<K> for Selector<K>
where
    K: OpaqueKey,
{
    fn from(key: K) -> Self {
        Selector::ByKey(key)
    }
}

impl<K> From<usize> for Selector<K> {
    fn from(index: usize) -> Self {
        Selector::ByIndex(index)
    }
}

/// Half-edge graph representation of a mesh.
///
/// Provides topological data in the form of vertices, arcs, edges, and faces.
/// An arc is directed from one vertex to another, with an opposing arc joining
/// the vertices in the other direction.
///
/// `MeshGraph`s expose topological views, which can be used to traverse and
/// manipulate topology and geometry in the graph.
///
/// See the module documentation for more details.
pub struct MeshGraph<G = (N64, N64, N64)>
where
    G: GraphGeometry,
{
    core: OwnedCore<G>,
}

impl<G> MeshGraph<G>
where
    G: GraphGeometry,
{
    /// Creates an empty `MeshGraph`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use plexus::graph::MeshGraph;
    ///
    /// let mut graph = MeshGraph::<()>::new();
    /// ```
    pub fn new() -> Self {
        MeshGraph::from(
            Core::empty()
                .bind(StorageProxy::<VertexPayload<G>>::new())
                .bind(StorageProxy::<ArcPayload<G>>::new())
                .bind(StorageProxy::<EdgePayload<G>>::new())
                .bind(StorageProxy::<FacePayload<G>>::new()),
        )
    }

    /// Gets the number of vertices in the graph.
    pub fn vertex_count(&self) -> usize {
        self.as_vertex_storage().len()
    }

    /// Gets an immutable view of the vertex with the given key.
    pub fn vertex(&self, key: VertexKey) -> Option<VertexView<&Self, G>> {
        (key, self).into_view()
    }

    /// Gets a mutable view of the vertex with the given key.
    pub fn vertex_mut(&mut self, key: VertexKey) -> Option<VertexView<&mut Self, G>> {
        (key, self).into_view()
    }

    // TODO: Return `Clone + Iterator`.
    /// Gets an iterator of immutable views over the vertices in the graph.
    pub fn vertices(&self) -> impl Iterator<Item = VertexView<&Self, G>> {
        self.as_vertex_storage()
            .keys()
            .map(move |key| (key, self).into_view().unwrap())
    }

    /// Gets an iterator of orphan views over the vertices in the graph.
    ///
    /// Because this only yields orphan views, only geometry can be mutated.
    /// For topological mutations, collect the necessary keys and use
    /// `vertex_mut` instead.
    pub fn orphan_vertices(&mut self) -> impl Iterator<Item = OrphanVertexView<G>> {
        self.as_vertex_storage_mut()
            .iter_mut()
            .map(|(key, source)| OrphanView::from_keyed_source_unchecked((key, source)))
            .map(|view| view.into())
    }

    /// Gets the number of arcs in the graph.
    pub fn arc_count(&self) -> usize {
        self.as_arc_storage().len()
    }

    /// Gets an immutable view of the arc with the given key.
    pub fn arc(&self, key: ArcKey) -> Option<ArcView<&Self, G>> {
        (key, self).into_view()
    }

    /// Gets a mutable view of the arc with the given key.
    pub fn arc_mut(&mut self, key: ArcKey) -> Option<ArcView<&mut Self, G>> {
        (key, self).into_view()
    }

    // TODO: Return `Clone + Iterator`.
    /// Gets an iterator of immutable views over the arcs in the graph.
    pub fn arcs(&self) -> impl Iterator<Item = ArcView<&Self, G>> {
        self.as_arc_storage()
            .keys()
            .map(move |key| (key, self).into_view().unwrap())
    }

    /// Gets an iterator of orphan views over the arcs in the graph.
    ///
    /// Because this only yields orphan views, only geometry can be mutated.
    /// For topological mutations, collect the necessary keys and use
    /// `arc_mut` instead.
    pub fn orphan_arcs(&mut self) -> impl Iterator<Item = OrphanArcView<G>> {
        self.as_arc_storage_mut()
            .iter_mut()
            .map(|(key, source)| OrphanView::from_keyed_source_unchecked((key, source)))
            .map(|view| view.into())
    }

    /// Gets the number of edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.as_edge_storage().len()
    }

    /// Gets an immutable view of the edge with the given key.
    pub fn edge(&self, key: EdgeKey) -> Option<EdgeView<&Self, G>> {
        (key, self).into_view()
    }

    /// Gets a mutable view of the edge with the given key.
    pub fn edge_mut(&mut self, key: EdgeKey) -> Option<EdgeView<&mut Self, G>> {
        (key, self).into_view()
    }

    // TODO: Return `Clone + Iterator`.
    /// Gets an iterator of immutable views over the edges in the graph.
    pub fn edges(&self) -> impl Iterator<Item = EdgeView<&Self, G>> {
        self.as_edge_storage()
            .keys()
            .map(move |key| (key, self).into_view().unwrap())
    }

    /// Gets an iterator of orphan views over the edges in the graph.
    ///
    /// Because this only yields orphan views, only geometry can be mutated.
    /// For topological mutations, collect the necessary keys and use
    /// `edge_mut` instead.
    pub fn orphan_edges(&mut self) -> impl Iterator<Item = OrphanEdgeView<G>> {
        self.as_edge_storage_mut()
            .iter_mut()
            .map(|(key, source)| OrphanView::from_keyed_source_unchecked((key, source)))
            .map(|view| view.into())
    }

    /// Gets the number of faces in the graph.
    pub fn face_count(&self) -> usize {
        self.as_face_storage().len()
    }

    /// Gets an immutable view of the face with the given key.
    pub fn face(&self, key: FaceKey) -> Option<FaceView<&Self, G>> {
        (key, self).into_view()
    }

    /// Gets a mutable view of the face with the given key.
    pub fn face_mut(&mut self, key: FaceKey) -> Option<FaceView<&mut Self, G>> {
        (key, self).into_view()
    }

    // TODO: Return `Clone + Iterator`.
    /// Gets an iterator of immutable views over the faces in the graph.
    pub fn faces(&self) -> impl Iterator<Item = FaceView<&Self, G>> {
        self.as_face_storage()
            .keys()
            .map(move |key| (key, self).into_view().unwrap())
    }

    /// Gets an iterator of orphan views over the faces in the graph.
    ///
    /// Because this only yields orphan views, only geometry can be mutated.
    /// For topological mutations, collect the necessary keys and use
    /// `face_mut` instead.
    pub fn orphan_faces(&mut self) -> impl Iterator<Item = OrphanFaceView<G>> {
        self.as_face_storage_mut()
            .iter_mut()
            .map(|(key, source)| OrphanView::from_keyed_source_unchecked((key, source)))
            .map(|view| view.into())
    }

    /// Gets an axis-aligned bounding box that encloses the mesh.
    pub fn aabb(&self) -> Aabb<VertexPosition<G>>
    where
        G::Vertex: AsPosition,
        VertexPosition<G>: EuclideanSpace,
    {
        Aabb::from_points(self.vertices().map(|vertex| *vertex.geometry.as_position()))
    }

    /// Gets the arity of the graph.
    ///
    /// If all faces in the graph have the same arity, then `Arity::Uniform` is
    /// returned with the singular arity of the graph. If the graph contains
    /// faces with differing arity, then `Arity::NonUniform` is returned with
    /// the minimum and maximum arity.
    ///
    /// `Arity::Uniform` is returned with zero if there are no faces in the
    /// graph.
    pub fn arity(&self) -> Arity {
        match self.faces().map(|face| face.arity()).minmax() {
            MinMaxResult::OneElement(arity) => Arity::Uniform(arity),
            MinMaxResult::MinMax(min, max) => Arity::NonUniform(min, max),
            _ => Arity::Uniform(0),
        }
    }

    /// Triangulates the mesh, tessellating all faces into triangles.
    pub fn triangulate(&mut self) {
        let faces = self.as_face_storage().keys().collect::<Vec<_>>();
        for face in faces {
            self.face_mut(face).unwrap().triangulate();
        }
    }

    pub fn smooth<T>(&mut self, factor: T)
    where
        T: Into<Scalar<VertexPosition<G>>>,
        G: VertexCentroid<Centroid = VertexPosition<G>>,
        G::Vertex: AsPosition,
        VertexPosition<G>: EuclideanSpace,
    {
        let factor = factor.into();
        let mut positions = HashMap::with_capacity(self.vertex_count());
        for vertex in self.vertices() {
            let position = *vertex.position();
            positions.insert(
                vertex.key(),
                position + ((vertex.centroid() - position) * factor),
            );
        }
        for mut vertex in self.orphan_vertices() {
            *vertex.geometry.as_position_mut() = positions.remove(&vertex.key()).unwrap();
        }
    }

    /// Creates a `MeshBuffer` from the graph.
    ///
    /// The buffer is created using the vertex geometry of each unique vertex.
    ///
    /// # Errors
    ///
    /// Returns an error if the mesh does not have constant arity that is
    /// compatible with the index buffer. Typically, a mesh is triangulated
    /// before being converted to a mesh buffer.
    pub fn to_mesh_buffer_by_vertex<A, N, H>(&self) -> Result<MeshBuffer<Flat<A, N>, H>, GraphError>
    where
        G::Vertex: IntoGeometry<H>,
        A: NonZero + typenum::Unsigned,
        N: Copy + Integer + NumCast + Unsigned,
    {
        self.to_mesh_buffer_by_vertex_with(|vertex| vertex.geometry.into_geometry())
    }

    /// Creates a `MeshBuffer` from the graph.
    ///
    /// The buffer is created using each unique vertex, which is converted into
    /// the buffer geometry by the given function.
    ///
    /// # Errors
    ///
    /// Returns an error if the mesh does not have constant arity that is
    /// compatible with the index buffer. Typically, a mesh is triangulated
    /// before being converted to a mesh buffer.
    pub fn to_mesh_buffer_by_vertex_with<A, N, H, F>(
        &self,
        mut f: F,
    ) -> Result<MeshBuffer<Flat<A, N>, H>, GraphError>
    where
        A: NonZero + typenum::Unsigned,
        N: Copy + Integer + NumCast + Unsigned,
        F: FnMut(VertexView<&Self, G>) -> H,
    {
        let (keys, vertices) = {
            let mut keys = HashMap::with_capacity(self.vertex_count());
            let mut vertices = Vec::with_capacity(self.vertex_count());
            for (n, vertex) in self.vertices().enumerate() {
                keys.insert(vertex.key(), n);
                vertices.push(f(vertex));
            }
            (keys, vertices)
        };
        let indices = {
            let arity = A::USIZE;
            let mut indices = Vec::with_capacity(arity * self.face_count());
            for face in self.faces() {
                if face.arity() != arity {
                    return Err(GraphError::ArityConflict {
                        expected: arity,
                        actual: face.arity(),
                    });
                }
                for vertex in face.vertices() {
                    indices.push(N::from(keys[&vertex.key()]).unwrap());
                }
            }
            indices
        };
        MeshBuffer::<Flat<_, _>, _>::from_raw_buffers(indices, vertices)
            .map_err(|error| error.into())
    }

    /// Creates a `MeshBuffer` from the graph.
    ///
    /// The buffer is created using the vertex geometry of each face. Shared
    /// vertices are included for each face to which they belong.
    ///
    /// # Errors
    ///
    /// Returns an error if the mesh does not have constant arity that is
    /// compatible with the index buffer. Typically, a mesh is triangulated
    /// before being converted to a mesh buffer.
    pub fn to_mesh_buffer_by_face<A, N, H>(&self) -> Result<MeshBuffer<Flat<A, N>, H>, GraphError>
    where
        G::Vertex: IntoGeometry<H>,
        A: NonZero + typenum::Unsigned,
        N: Copy + Integer + NumCast + Unsigned,
    {
        self.to_mesh_buffer_by_face_with(|_, vertex| vertex.geometry.into_geometry())
    }

    /// Creates a `MeshBuffer` from the graph.
    ///
    /// The buffer is created from each face, which is converted into the
    /// buffer geometry by the given function.
    ///
    /// # Errors
    ///
    /// Returns an error if the mesh does not have constant arity that is
    /// compatible with the index buffer. Typically, a mesh is triangulated
    /// before being converted to a mesh buffer.
    pub fn to_mesh_buffer_by_face_with<A, N, H, F>(
        &self,
        mut f: F,
    ) -> Result<MeshBuffer<Flat<A, N>, H>, GraphError>
    where
        A: NonZero + typenum::Unsigned,
        N: Copy + Integer + NumCast + Unsigned,
        F: FnMut(FaceView<&Self, G>, VertexView<&Self, G>) -> H,
    {
        let vertices = {
            let arity = A::USIZE;
            let mut vertices = Vec::with_capacity(arity * self.face_count());
            for face in self.faces() {
                if face.arity() != arity {
                    return Err(GraphError::ArityConflict {
                        expected: arity,
                        actual: face.arity(),
                    });
                }
                for vertex in face.vertices() {
                    // TODO: Can some sort of dereference be used here?
                    vertices.push(f(face, vertex));
                }
            }
            vertices
        };
        MeshBuffer::<Flat<_, _>, _>::from_raw_buffers(
            // TODO: Cannot use the bound `N: Step`, which is unstable.
            (0..vertices.len()).map(|index| N::from(index).unwrap()),
            vertices,
        )
        .map_err(|error| error.into())
    }
}

impl<G> AsStorage<VertexPayload<G>> for MeshGraph<G>
where
    G: GraphGeometry,
{
    fn as_storage(&self) -> &StorageProxy<VertexPayload<G>> {
        self.core.as_vertex_storage()
    }
}

impl<G> AsStorage<ArcPayload<G>> for MeshGraph<G>
where
    G: GraphGeometry,
{
    fn as_storage(&self) -> &StorageProxy<ArcPayload<G>> {
        self.core.as_arc_storage()
    }
}

impl<G> AsStorage<EdgePayload<G>> for MeshGraph<G>
where
    G: GraphGeometry,
{
    fn as_storage(&self) -> &StorageProxy<EdgePayload<G>> {
        self.core.as_edge_storage()
    }
}

impl<G> AsStorage<FacePayload<G>> for MeshGraph<G>
where
    G: GraphGeometry,
{
    fn as_storage(&self) -> &StorageProxy<FacePayload<G>> {
        self.core.as_face_storage()
    }
}

impl<G> AsStorageMut<VertexPayload<G>> for MeshGraph<G>
where
    G: GraphGeometry,
{
    fn as_storage_mut(&mut self) -> &mut StorageProxy<VertexPayload<G>> {
        self.core.as_vertex_storage_mut()
    }
}

impl<G> AsStorageMut<ArcPayload<G>> for MeshGraph<G>
where
    G: GraphGeometry,
{
    fn as_storage_mut(&mut self) -> &mut StorageProxy<ArcPayload<G>> {
        self.core.as_arc_storage_mut()
    }
}

impl<G> AsStorageMut<EdgePayload<G>> for MeshGraph<G>
where
    G: GraphGeometry,
{
    fn as_storage_mut(&mut self) -> &mut StorageProxy<EdgePayload<G>> {
        self.core.as_edge_storage_mut()
    }
}

impl<G> AsStorageMut<FacePayload<G>> for MeshGraph<G>
where
    G: GraphGeometry,
{
    fn as_storage_mut(&mut self) -> &mut StorageProxy<FacePayload<G>> {
        self.core.as_face_storage_mut()
    }
}

impl<G> Consistent for MeshGraph<G> where G: GraphGeometry {}

impl<G> Default for MeshGraph<G>
where
    G: GraphGeometry,
{
    fn default() -> Self {
        MeshGraph::new()
    }
}

impl<G> From<OwnedCore<G>> for MeshGraph<G>
where
    G: GraphGeometry,
{
    fn from(core: OwnedCore<G>) -> Self {
        MeshGraph { core }
    }
}

impl<E, G> FromEncoding<E> for MeshGraph<G>
where
    G: GraphGeometry,
    E: FaceDecoder + VertexDecoder,
    E::Face: IntoGeometry<G::Face>,
    E::Vertex: IntoGeometry<G::Vertex>,
{
    type Error = GraphError;

    fn from_encoding(
        vertices: <E as VertexDecoder>::Output,
        faces: <E as FaceDecoder>::Output,
    ) -> Result<Self, Self::Error> {
        let mut mutation = Mutation::mutate(MeshGraph::new());
        let keys = vertices
            .into_iter()
            .map(|geometry| mutation.insert_vertex(geometry.into_geometry()))
            .collect::<Vec<_>>();
        for (perimeter, geometry) in faces {
            let perimeter = perimeter
                .into_iter()
                .map(|index| keys[index])
                .collect::<SmallVec<[_; 4]>>();
            mutation.insert_face(
                perimeter.as_slice(),
                (Default::default(), geometry.into_geometry()),
            )?;
        }
        mutation.commit()
    }
}

impl<G, P> FromIndexer<P, P> for MeshGraph<G>
where
    G: GraphGeometry,
    P: Map<usize> + Polygonal,
    P::Output: Grouping<Item = P::Output> + IntoVertices + Polygonal<Vertex = usize>,
    P::Vertex: IntoGeometry<G::Vertex>,
    Vec<P::Output>: IndexBuffer<P::Output, Index = usize>,
{
    type Error = GraphError;

    fn from_indexer<I, N>(input: I, indexer: N) -> Result<Self, Self::Error>
    where
        I: IntoIterator<Item = P>,
        N: Indexer<P, P::Vertex>,
    {
        let mut mutation = Mutation::mutate(MeshGraph::new());
        let (indices, vertices) = input.into_iter().index_vertices(indexer);
        let vertices = vertices
            .into_iter()
            .map(|vertex| mutation.insert_vertex(vertex.into_geometry()))
            .collect::<Vec<_>>();
        for face in indices {
            let perimeter = face
                .into_vertices()
                .into_iter()
                .map(|index| vertices[index])
                .collect::<SmallVec<[_; 4]>>();
            mutation.insert_face(&perimeter, Default::default())?;
        }
        mutation.commit()
    }
}

impl<G, P> FromIterator<P> for MeshGraph<G>
where
    G: GraphGeometry,
    P: Polygonal,
    P::Vertex: Clone + Eq + Hash + IntoGeometry<G::Vertex>,
    Self: FromIndexer<P, P>,
{
    fn from_iter<I>(input: I) -> Self
    where
        I: IntoIterator<Item = P>,
    {
        Self::from_indexer(input, HashIndexer::default()).unwrap_or_else(|_| Self::default())
    }
}

impl<P, G, H> FromRawBuffers<P, H> for MeshGraph<G>
where
    P: IntoVertices + Polygonal,
    P::Vertex: Integer + ToPrimitive + Unsigned,
    G: GraphGeometry,
    H: IntoGeometry<G::Vertex>,
{
    type Error = GraphError;

    fn from_raw_buffers<I, J>(indices: I, vertices: J) -> Result<Self, Self::Error>
    where
        I: IntoIterator<Item = P>,
        J: IntoIterator<Item = H>,
    {
        let mut mutation = Mutation::mutate(MeshGraph::new());
        let vertices = vertices
            .into_iter()
            .map(|vertex| mutation.insert_vertex(vertex.into_geometry()))
            .collect::<Vec<_>>();
        for face in indices {
            let mut perimeter = SmallVec::<[_; 4]>::with_capacity(face.arity());
            for index in face.into_vertices() {
                let index = <usize as NumCast>::from(index).unwrap();
                perimeter.push(
                    *vertices
                        .get(index)
                        .ok_or_else(|| GraphError::TopologyNotFound)?,
                );
            }
            mutation.insert_face(&perimeter, Default::default())?;
        }
        mutation.commit()
    }
}

impl<N, G, H> FromRawBuffersWithArity<N, H> for MeshGraph<G>
where
    N: Integer + ToPrimitive + Unsigned,
    G: GraphGeometry,
    H: IntoGeometry<G::Vertex>,
{
    type Error = GraphError;

    /// Creates a `MeshGraph` from raw index and vertex buffers. The arity of
    /// the polygons in the index buffer must be known and constant.
    ///
    /// # Errors
    ///
    /// Returns an error if the arity of the index buffer is not constant, any
    /// index is out of bounds, or there is an error inserting topology into
    /// the mesh.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # extern crate nalgebra;
    /// # extern crate plexus;
    /// #
    /// use nalgebra::Point3;
    /// use plexus::graph::MeshGraph;
    /// use plexus::index::{Flat3, LruIndexer};
    /// use plexus::prelude::*;
    /// use plexus::primitive::generate::Position;
    /// use plexus::primitive::sphere::UvSphere;
    ///
    /// # fn main() {
    /// let (indices, positions) = UvSphere::new(16, 16)
    ///     .polygons::<Position<Point3<f64>>>()
    ///     .triangulate()
    ///     .index_vertices::<Flat3, _>(LruIndexer::with_capacity(256));
    /// let mut graph =
    ///     MeshGraph::<Point3<f64>>::from_raw_buffers_with_arity(indices, positions, 3).unwrap();
    /// # }
    /// ```
    fn from_raw_buffers_with_arity<I, J>(
        indices: I,
        vertices: J,
        arity: usize,
    ) -> Result<Self, Self::Error>
    where
        I: IntoIterator<Item = N>,
        J: IntoIterator<Item = H>,
    {
        if arity < 3 {
            return Err(GraphError::ArityNonPolygonal);
        }
        let mut mutation = Mutation::mutate(MeshGraph::new());
        let vertices = vertices
            .into_iter()
            .map(|vertex| mutation.insert_vertex(vertex.into_geometry()))
            .collect::<Vec<_>>();
        for face in &indices
            .into_iter()
            .map(|index| <usize as NumCast>::from(index).unwrap())
            .chunks(arity)
        {
            let face = face.collect::<Vec<_>>();
            if face.len() != arity {
                // Index buffer length is not a multiple of arity.
                return Err(GraphError::ArityConflict {
                    expected: arity,
                    actual: face.len(),
                });
            }
            let mut perimeter = SmallVec::<[_; 4]>::with_capacity(arity);
            for index in face {
                perimeter.push(
                    *vertices
                        .get(index)
                        .ok_or_else(|| GraphError::TopologyNotFound)?,
                );
            }
            mutation.insert_face(&perimeter, Default::default())?;
        }
        mutation.commit()
    }
}

impl<G> Into<OwnedCore<G>> for MeshGraph<G>
where
    G: GraphGeometry,
{
    fn into(self) -> OwnedCore<G> {
        let MeshGraph { core, .. } = self;
        core
    }
}

impl<A, N, H, G> TryFrom<MeshBuffer<Flat<A, N>, H>> for MeshGraph<G>
where
    A: NonZero + typenum::Unsigned,
    N: Copy + Integer + NumCast + Unsigned,
    H: Clone + IntoGeometry<G::Vertex>,
    G: GraphGeometry,
{
    type Error = GraphError;

    /// Creates a `MeshGraph` from a flat `MeshBuffer`. The arity of the
    /// polygons in the index buffer must be known and constant.
    ///
    /// # Errors
    ///
    /// Returns an error if a `MeshGraph` cannot represent the topology in the
    /// `MeshBuffer`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # extern crate nalgebra;
    /// # extern crate plexus;
    /// #
    /// use nalgebra::Point2;
    /// use plexus::buffer::MeshBuffer;
    /// use plexus::graph::MeshGraph;
    /// use plexus::index::Flat4;
    /// use plexus::prelude::*;
    /// use std::convert::TryFrom;
    ///
    /// # fn main() {
    /// let buffer = MeshBuffer::<Flat4, _>::from_raw_buffers(
    ///     vec![0u64, 1, 2, 3],
    ///     vec![(0.0f64, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
    /// )
    /// .unwrap();
    /// let mut graph = MeshGraph::<Point2<f64>>::try_from(buffer).unwrap();
    /// # }
    /// ```
    fn try_from(buffer: MeshBuffer<Flat<A, N>, H>) -> Result<Self, Self::Error> {
        let arity = match buffer.arity() {
            Arity::Uniform(arity) => arity,
            _ => panic!("non-uniform flat index buffer arity"),
        };
        let (indices, vertices) = buffer.into_raw_buffers();
        MeshGraph::from_raw_buffers_with_arity(indices, vertices, arity)
    }
}

impl<P, H, G> TryFrom<MeshBuffer<P, H>> for MeshGraph<G>
where
    P: Grouping<Item = P> + IntoVertices + Polygonal,
    P::Vertex: Copy + Integer + NumCast + Unsigned,
    H: Clone + IntoGeometry<G::Vertex>,
    G: GraphGeometry,
{
    type Error = GraphError;

    /// Creates a `MeshGraph` from a structured `MeshBuffer`.
    ///
    /// # Errors
    ///
    /// Returns an error if a `MeshGraph` cannot represent the topology in the
    /// `MeshBuffer`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # extern crate nalgebra;
    /// # extern crate plexus;
    /// #
    /// use nalgebra::Point2;
    /// use plexus::buffer::MeshBuffer;
    /// use plexus::graph::MeshGraph;
    /// use plexus::prelude::*;
    /// use plexus::primitive::Tetragon;
    /// use std::convert::TryFrom;
    ///
    /// # fn main() {
    /// let buffer = MeshBuffer::<Tetragon<u64>, _>::from_raw_buffers(
    ///     vec![Tetragon::new(0u64, 1, 2, 3)],
    ///     vec![(0.0f64, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
    /// )
    /// .unwrap();
    /// let mut graph = MeshGraph::<Point2<f64>>::try_from(buffer).unwrap();
    /// # }
    /// ```
    fn try_from(buffer: MeshBuffer<P, H>) -> Result<Self, Self::Error> {
        let (indices, vertices) = buffer.into_raw_buffers();
        MeshGraph::from_raw_buffers(indices, vertices)
    }
}

#[cfg(test)]
mod tests {
    use decorum::N64;
    use nalgebra::{Point3, Vector3};
    use num::Zero;
    use typenum::U3;

    use crate::graph::{GraphError, GraphGeometry, MeshGraph};
    use crate::prelude::*;
    use crate::primitive::generate::Position;
    use crate::primitive::sphere::UvSphere;

    type E3 = Point3<N64>;

    #[test]
    fn collect_topology_into_mesh() {
        let graph = UvSphere::new(3, 2)
            .polygons::<Position<E3>>() // 6 triangles, 18 vertices.
            .collect::<MeshGraph<Point3<f64>>>();

        assert_eq!(5, graph.vertex_count());
        assert_eq!(18, graph.arc_count());
        assert_eq!(6, graph.face_count());
    }

    #[test]
    fn iterate_mesh_topology() {
        let mut graph = UvSphere::new(4, 2)
            .polygons::<Position<E3>>() // 8 triangles, 24 vertices.
            .collect::<MeshGraph<Point3<f64>>>();

        assert_eq!(6, graph.vertices().count());
        assert_eq!(24, graph.arcs().count());
        assert_eq!(8, graph.faces().count());
        for vertex in graph.vertices() {
            // Every vertex is connected to 4 triangles with 4 (incoming) arcs.
            // Traversal of topology should be possible.
            assert_eq!(4, vertex.incoming_arcs().count());
        }
        for mut vertex in graph.orphan_vertices() {
            // Geometry should be mutable.
            vertex.geometry += Vector3::zero();
        }
    }

    #[test]
    fn non_manifold_error_deferred() {
        let graph = UvSphere::new(32, 32)
            .polygons::<Position<E3>>()
            .triangulate()
            .collect::<MeshGraph<Point3<f64>>>();
        // This conversion will join faces by a single vertex, but ultimately
        // creates a manifold.
        graph
            .to_mesh_buffer_by_face_with::<U3, usize, _, _>(|_, vertex| vertex.geometry)
            .unwrap();
    }

    #[test]
    fn error_on_non_manifold_mesh() {
        // Construct a mesh with a "fan" of three triangles sharing the same
        // arc along the Z-axis. The edge would have three associated faces,
        // which should not be possible.
        let graph = MeshGraph::<Point3<i32>>::from_raw_buffers_with_arity(
            vec![0u32, 1, 2, 0, 1, 3, 0, 1, 4],
            vec![(0, 0, 1), (0, 0, -1), (1, 0, 0), (0, 1, 0), (1, 1, 0)],
            3,
        );

        assert_eq!(graph.err().unwrap(), GraphError::TopologyConflict);
    }

    // This test is a sanity check for mesh iterators, topological views, and
    // the unsafe transmutations used to coerce lifetimes.
    #[test]
    fn read_write_geometry_ref() {
        struct ValueGeometry;

        impl GraphGeometry for ValueGeometry {
            type Vertex = Point3<f64>;
            type Arc = ();
            type Edge = ();
            type Face = f64;
        }

        // Create a mesh with a floating point value associated with each face.
        // Use a mutable iterator to write to the geometry of each face.
        let mut graph = UvSphere::new(4, 4)
            .polygons::<Position<E3>>()
            .collect::<MeshGraph<ValueGeometry>>();
        let value = 3.14;
        for mut face in graph.orphan_faces() {
            face.geometry = value;
        }

        // Read the geometry of each face using an immutable iterator to ensure
        // it is what we expect.
        for face in graph.faces() {
            assert_eq!(value, face.geometry);
        }
    }
}