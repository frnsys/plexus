use arrayvec::ArrayVec;
use fool::BoolExt;
use std::marker::PhantomData;
use std::mem;
use std::ops::{Deref, DerefMut};
use theon::space::{EuclideanSpace, Scalar, Vector};
use theon::AsPosition;

use crate::graph::borrow::{Reborrow, ReborrowMut};
use crate::graph::geometry::{ArcNormal, EdgeMidpoint, GraphGeometry, VertexPosition};
use crate::graph::mutation::edge::{
    self, ArcBridgeCache, ArcExtrudeCache, EdgeRemoveCache, EdgeSplitCache,
};
use crate::graph::mutation::{Consistent, Mutable, Mutation};
use crate::graph::storage::key::{ArcKey, EdgeKey, FaceKey, VertexKey};
use crate::graph::storage::payload::{Arc, Edge, Face, Vertex};
use crate::graph::storage::{AsStorage, AsStorageMut, StorageProxy};
use crate::graph::view::face::{FaceOrphan, FaceView, RingView};
use crate::graph::view::path::PathView;
use crate::graph::view::vertex::{VertexOrphan, VertexView};
use crate::graph::view::{Binding, Orphan, View};
use crate::graph::{GraphError, OptionExt as _, ResultExt as _, Selector};
use crate::transact::{Mutate, Transact};

/// Edge-like structure. Abstracts arcs and edges.
///
/// Types implementing this trait participate in a composite edge and can be
/// converted into an arc or edge that is a part of that composite edge. This
/// trait allows edge structures to be abstracted.
pub trait Edgoid<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + AsStorage<Edge<G>> + Consistent,
    G: GraphGeometry,
{
    fn into_arc(self) -> ArcView<M, G>;

    fn into_edge(self) -> EdgeView<M, G>;
}

/// View of an arc in a graph.
///
/// Provides traversals, queries, and mutations related to arcs in a graph. See
/// the module documentation for more information about topological views.
///
/// Arcs provide the connectivity information within a `MeshGraph` and are the
/// primary mechanism for traversing its topology. Moreover, most edge-like
/// operations are exposed by arcs, because they are directed and therefore can
/// emit deterministic results (this is not true of edges).
///
/// An arc from a vertex $A$ to a vertex $B$ is notated $\overrightarrow{AB}$.
/// This is shorthand for the path notation $\overrightarrow{\\{A,B\\}}$.
///
/// # Examples
///
/// Traversing a graph of a cube via its arcs to find an opposing face:
///
/// ```rust
/// # extern crate decorum;
/// # extern crate nalgebra;
/// # extern crate plexus;
/// #
/// use decorum::N64;
/// use nalgebra::Point3;
/// use plexus::graph::MeshGraph;
/// use plexus::index::HashIndexer;
/// use plexus::prelude::*;
/// use plexus::primitive::cube::Cube;
/// use plexus::primitive::generate::Position;
///
/// let mut graph = Cube::new()
///     .polygons::<Position<Point3<N64>>>()
///     .collect_with_indexer::<MeshGraph<Point3<N64>>, _>(HashIndexer::default())
///     .unwrap();
///
/// let face = graph.faces().nth(0).unwrap();
/// let opposite = face
///     .into_arc()
///     .into_opposite_arc()
///     .into_next_arc()
///     .into_next_arc()
///     .into_opposite_arc()
///     .into_face()
///     .unwrap();
/// ```
pub struct ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>>,
    G: GraphGeometry,
{
    inner: View<M, Arc<G>>,
}

impl<M, G> ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>>,
    G: GraphGeometry,
{
    fn into_inner(self) -> View<M, Arc<G>> {
        self.into()
    }

    fn interior_reborrow(&self) -> ArcView<&M::Target, G> {
        self.inner.interior_reborrow().into()
    }

    /// Returns `true` if this is a boundary arc.
    ///
    /// A boundary arc has no associated face.
    pub fn is_boundary_arc(&self) -> bool {
        self.face.is_none()
    }
}

impl<M, G> ArcView<M, G>
where
    M: ReborrowMut,
    M::Target: AsStorage<Arc<G>>,
    G: GraphGeometry,
{
    fn interior_reborrow_mut(&mut self) -> ArcView<&mut M::Target, G> {
        self.inner.interior_reborrow_mut().into()
    }
}

impl<'a, M, G> ArcView<&'a mut M, G>
where
    M: 'a + AsStorageMut<Arc<G>>,
    G: 'a + GraphGeometry,
{
    /// Converts a mutable view into an immutable view.
    ///
    /// This is useful when mutations are not (or no longer) needed and mutual
    /// access is desired.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # extern crate nalgebra;
    /// # extern crate plexus;
    /// #
    /// use nalgebra::Point2;
    /// use plexus::graph::MeshGraph;
    /// use plexus::prelude::*;
    ///
    /// let mut graph = MeshGraph::<Point2<f64>>::from_raw_buffers_with_arity(
    ///     vec![0u32, 1, 2, 3],
    ///     vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
    ///     4,
    /// )
    /// .unwrap();
    /// let key = graph
    ///     .arcs()
    ///     .find(|arc| arc.is_boundary_arc())
    ///     .unwrap()
    ///     .key();
    /// let arc = graph.arc_mut(key).unwrap().extrude(1.0).unwrap().into_ref();
    ///
    /// // This would not be possible without conversion into an immutable view.
    /// let _ = arc.into_next_arc().into_next_arc().into_face();
    /// let _ = arc.into_opposite_arc().into_face();
    /// ```
    pub fn into_ref(self) -> ArcView<&'a M, G> {
        self.into_inner().into_ref().into()
    }
}

/// Reachable API.
impl<M, G> ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>>,
    G: GraphGeometry,
{
    pub(in crate::graph) fn into_reachable_boundary_arc(self) -> Option<Self> {
        if self.is_boundary_arc() {
            Some(self)
        }
        else {
            self.into_reachable_opposite_arc()
                .and_then(|opposite| opposite.is_boundary_arc().some(opposite))
        }
    }

    pub(in crate::graph) fn into_reachable_opposite_arc(self) -> Option<Self> {
        let key = self.key().into_opposite();
        self.into_inner().rebind_into(key)
    }

    pub(in crate::graph) fn into_reachable_next_arc(self) -> Option<Self> {
        let inner = self.into_inner();
        let key = inner.next;
        key.and_then(move |key| inner.rebind_into(key))
    }

    pub(in crate::graph) fn into_reachable_previous_arc(self) -> Option<Self> {
        let inner = self.into_inner();
        let key = inner.previous;
        key.and_then(move |key| inner.rebind_into(key))
    }

    pub(in crate::graph) fn reachable_boundary_arc(&self) -> Option<ArcView<&M::Target, G>> {
        if self.is_boundary_arc() {
            Some(self.interior_reborrow())
        }
        else {
            self.reachable_opposite_arc()
                .and_then(|opposite| opposite.is_boundary_arc().some_with(|| opposite))
        }
    }

    pub(in crate::graph) fn reachable_opposite_arc(&self) -> Option<ArcView<&M::Target, G>> {
        let key = self.key().into_opposite();
        self.inner.interior_reborrow().rebind_into(key)
    }

    pub(in crate::graph) fn reachable_next_arc(&self) -> Option<ArcView<&M::Target, G>> {
        self.next
            .and_then(|key| self.inner.interior_reborrow().rebind_into(key))
    }

    pub(in crate::graph) fn reachable_previous_arc(&self) -> Option<ArcView<&M::Target, G>> {
        self.previous
            .and_then(|key| self.inner.interior_reborrow().rebind_into(key))
    }
}

impl<M, G> ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + Consistent,
    G: GraphGeometry,
{
    /// Converts the arc into its ring.
    pub fn into_ring(self) -> RingView<M, G> {
        let (storage, key) = self.into_inner().unbind();
        View::bind(storage, key).expect_consistent().into()
    }

    /// Returns the arc if it is a boundary arc, otherwise `None`.
    pub fn into_boundary_arc(self) -> Option<Self> {
        self.into_reachable_boundary_arc()
    }

    /// Converts the arc into its opposite arc.
    pub fn into_opposite_arc(self) -> Self {
        self.into_reachable_opposite_arc().expect_consistent()
    }

    /// Converts the arc into its next arc.
    pub fn into_next_arc(self) -> Self {
        self.into_reachable_next_arc().expect_consistent()
    }

    /// Converts the arc into its previous arc.
    pub fn into_previous_arc(self) -> Self {
        self.into_reachable_previous_arc().expect_consistent()
    }

    /// Gets the ring of the arc.
    pub fn ring(&self) -> RingView<&M::Target, G> {
        let (storage, key) = self.inner.interior_reborrow().unbind();
        View::bind_into(storage, key).expect_consistent()
    }

    /// Returns the same arc if it is a boundary arc, otherwise `None`.
    pub fn boundary_arc(&self) -> Option<ArcView<&M::Target, G>> {
        self.reachable_boundary_arc()
    }

    /// Gets the opposite arc.
    pub fn opposite_arc(&self) -> ArcView<&M::Target, G> {
        self.reachable_opposite_arc().expect_consistent()
    }

    /// Gets the next arc.
    pub fn next_arc(&self) -> ArcView<&M::Target, G> {
        self.reachable_next_arc().expect_consistent()
    }

    /// Gets the previous arc.
    pub fn previous_arc(&self) -> ArcView<&M::Target, G> {
        self.reachable_previous_arc().expect_consistent()
    }
}

/// Reachable API.
impl<M, G> ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + AsStorage<Vertex<G>>,
    G: GraphGeometry,
{
    pub(in crate::graph) fn into_reachable_source_vertex(self) -> Option<VertexView<M, G>> {
        let (key, _) = self.key().into();
        self.into_inner().rebind_into(key)
    }

    pub(in crate::graph) fn into_reachable_destination_vertex(self) -> Option<VertexView<M, G>> {
        let (_, key) = self.key().into();
        self.into_inner().rebind_into(key)
    }

    pub(in crate::graph) fn reachable_source_vertex(&self) -> Option<VertexView<&M::Target, G>> {
        let (key, _) = self.key().into();
        self.inner.interior_reborrow().rebind_into(key)
    }

    pub(in crate::graph) fn reachable_destination_vertex(
        &self,
    ) -> Option<VertexView<&M::Target, G>> {
        let (_, key) = self.key().into();
        self.inner.interior_reborrow().rebind_into(key)
    }
}

impl<M, G> ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + AsStorage<Vertex<G>> + Consistent,
    G: GraphGeometry,
{
    pub fn into_path(self) -> PathView<M, G> {
        let (storage, ab) = self.into_inner().unbind();
        let (a, b) = ab.into();
        PathView::try_from_keys(storage, &[a, b]).unwrap()
    }

    pub fn path(&self) -> PathView<&M::Target, G> {
        self.interior_reborrow().into_path()
    }

    /// Converts the arc into its source vertex.
    pub fn into_source_vertex(self) -> VertexView<M, G> {
        self.into_reachable_source_vertex().expect_consistent()
    }

    /// Converts the arc into its destination vertex.
    pub fn into_destination_vertex(self) -> VertexView<M, G> {
        self.into_reachable_destination_vertex().expect_consistent()
    }

    /// Gets the source vertex of the arc.
    pub fn source_vertex(&self) -> VertexView<&M::Target, G> {
        self.reachable_source_vertex().expect_consistent()
    }

    /// Gets the destination vertex of the arc.
    pub fn destination_vertex(&self) -> VertexView<&M::Target, G> {
        self.reachable_destination_vertex().expect_consistent()
    }
}

/// Reachable API.
impl<M, G> ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + AsStorage<Edge<G>>,
    G: GraphGeometry,
{
    pub(in crate::graph) fn into_reachable_edge(self) -> Option<EdgeView<M, G>> {
        let inner = self.into_inner();
        let key = inner.edge;
        key.and_then(move |key| inner.rebind_into(key))
    }

    pub(in crate::graph) fn reachable_edge(&self) -> Option<EdgeView<&M::Target, G>> {
        self.edge
            .and_then(|key| self.inner.interior_reborrow().rebind_into(key))
    }
}

impl<M, G> ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + AsStorage<Edge<G>> + Consistent,
    G: GraphGeometry,
{
    /// Converts the arc into its edge.
    pub fn into_edge(self) -> EdgeView<M, G> {
        self.into_reachable_edge().expect_consistent()
    }

    /// Gets the edge of the arc.
    pub fn edge(&self) -> EdgeView<&M::Target, G> {
        self.reachable_edge().expect_consistent()
    }
}

/// Reachable API.
impl<M, G> ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + AsStorage<Face<G>>,
    G: GraphGeometry,
{
    pub(in crate::graph) fn into_reachable_face(self) -> Option<FaceView<M, G>> {
        let inner = self.into_inner();
        let key = inner.face;
        key.and_then(move |key| inner.rebind_into(key))
    }

    pub(in crate::graph) fn reachable_face(&self) -> Option<FaceView<&M::Target, G>> {
        self.face
            .and_then(|key| self.inner.interior_reborrow().rebind_into(key))
    }
}

impl<M, G> ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + AsStorage<Face<G>> + Consistent,
    G: GraphGeometry,
{
    /// Converts the arc into its face.
    ///
    /// If this is a boundary arc, then `None` is returned.
    pub fn into_face(self) -> Option<FaceView<M, G>> {
        self.into_reachable_face()
    }

    /// Gets the face of this arc.
    ///
    /// If this is a boundary arc, then `None` is returned.
    pub fn face(&self) -> Option<FaceView<&M::Target, G>> {
        self.reachable_face()
    }
}

impl<M, G> ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + AsStorage<Vertex<G>> + Consistent,
    G: GraphGeometry,
{
    /// Gets an iterator of views over the vertices connected by the arc.
    pub fn vertices(&self) -> impl Clone + ExactSizeIterator<Item = VertexView<&M::Target, G>> {
        VertexCirculator::from(self.interior_reborrow())
    }
}

impl<M, G> ArcView<M, G>
where
    M: ReborrowMut,
    M::Target: AsStorage<Arc<G>> + AsStorageMut<Vertex<G>> + Consistent,
    G: GraphGeometry,
{
    /// Gets an iterator of orphan views over the vertices connected by the
    /// arc.
    pub fn vertex_orphans(&mut self) -> impl ExactSizeIterator<Item = VertexOrphan<G>> {
        VertexCirculator::from(self.interior_reborrow_mut())
    }
}

impl<M, G> ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + AsStorage<Face<G>> + Consistent,
    G: GraphGeometry,
{
    /// Gets an iterator of views over the faces connected to the arc.
    pub fn faces(&self) -> impl Clone + ExactSizeIterator<Item = FaceView<&M::Target, G>> {
        FaceCirculator::from(self.interior_reborrow())
    }
}

impl<M, G> ArcView<M, G>
where
    M: ReborrowMut,
    M::Target: AsStorage<Arc<G>> + AsStorageMut<Face<G>> + Consistent,
    G: GraphGeometry,
{
    /// Gets an iterator of orphan views over the faces connected to the arc.
    pub fn face_orphans(&mut self) -> impl ExactSizeIterator<Item = FaceOrphan<G>> {
        FaceCirculator::from(self.interior_reborrow_mut())
    }
}

impl<M, G> ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + AsStorage<Vertex<G>> + Consistent,
    G: GraphGeometry,
{
    pub fn normal(&self) -> Vector<VertexPosition<G>>
    where
        G: ArcNormal,
        G::Vertex: AsPosition,
    {
        G::normal(self.interior_reborrow()).expect_consistent()
    }
}

impl<M, G> ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + AsStorage<Edge<G>> + AsStorage<Vertex<G>> + Consistent,
    G: GraphGeometry,
{
    pub fn midpoint(&self) -> VertexPosition<G>
    where
        G: EdgeMidpoint,
        G::Vertex: AsPosition,
    {
        G::midpoint(self.interior_reborrow()).expect_consistent()
    }
}

impl<'a, M, G> ArcView<&'a mut M, G>
where
    M: AsStorage<Arc<G>>
        + AsStorage<Edge<G>>
        + AsStorage<Face<G>>
        + AsStorage<Vertex<G>>
        + Default
        + Mutable<G>,
    G: 'a + GraphGeometry,
{
    /// Splits a composite edge into two neighboring edges that share a vertex.
    ///
    /// Splitting inserts a new vertex with the geometry provided by the given
    /// function. Splitting an arc $\overrightarrow{AB}$ returns a vertex $M$
    /// that subdivides the composite edge. The leading arc of $M$ is
    /// $\overrightarrow{MB}$ and is a part of the same ring as the initiating
    /// arc.
    ///
    /// Returns the inserted vertex.
    ///
    /// # Examples
    ///
    /// Split an edge in a graph with weighted vertices:
    ///
    /// ```rust
    /// use plexus::graph::{GraphGeometry, MeshGraph};
    /// use plexus::prelude::*;
    /// use plexus::primitive::NGon;
    ///
    /// pub enum Weight {}
    ///
    /// impl GraphGeometry for Weight {
    ///     type Vertex = f64;
    ///     type Arc = ();
    ///     type Edge = ();
    ///     type Face = ();
    /// }
    ///
    /// let mut graph =
    ///     MeshGraph::<Weight>::from_raw_buffers(vec![NGon([0usize, 1, 2])], vec![1.0, 2.0, 0.5])
    ///         .unwrap();
    /// let key = graph.arcs().nth(0).unwrap().key();
    /// let vertex = graph.arc_mut(key).unwrap().split_with(|| 0.1);
    /// ```
    pub fn split_with<F>(self, f: F) -> VertexView<&'a mut M, G>
    where
        F: FnOnce() -> G::Vertex,
    {
        let (storage, ab) = self.into_inner().unbind();
        let cache = EdgeSplitCache::snapshot(&storage, ab, f()).expect_consistent();
        Mutation::replace(storage, Default::default())
            .commit_with(move |mutation| edge::split_with_cache(mutation, cache))
            .map(|(storage, m)| View::bind_into(storage, m).expect_consistent())
            .expect_consistent()
    }

    /// Splits an edge (and its arcs) at the midpoint of the arc's vertices.
    ///
    /// Splitting inserts a new vertex with the geometry of the arc's source
    /// vertex but modified such that the positional data of the vertex is the
    /// computed midpoint of both of the arc's vertices.
    ///
    /// Splitting inserts a new vertex with the geometry provided by the given
    /// function. Splitting an arc $\overrightarrow{AB}$ returns a vertex $M$
    /// that subdivides the composite edge. The leading arc of $M$ is
    /// $\overrightarrow{MB}$ and is a part of the same ring as the initiating
    /// arc.
    ///
    /// This function is only available if a graph's geometry exposes
    /// positional data in its vertices and that data supports interpolation.
    /// See the `geometry` module.
    ///
    /// Returns the inserted vertex.
    ///
    /// # Examples
    ///
    /// Split an edge in a triangle at its midpoint:
    ///
    /// ```rust
    /// # extern crate nalgebra;
    /// # extern crate plexus;
    /// #
    /// use nalgebra::Point2;
    /// use plexus::graph::MeshGraph;
    /// use plexus::prelude::*;
    /// use plexus::primitive::Trigon;
    ///
    /// let mut graph = MeshGraph::<Point2<f64>>::from_raw_buffers(
    ///     vec![Trigon::new(0usize, 1, 2)],
    ///     vec![(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)],
    /// )
    /// .unwrap();
    /// let key = graph.arcs().nth(0).unwrap().key();
    /// let vertex = graph.arc_mut(key).unwrap().split_at_midpoint();
    /// ```
    pub fn split_at_midpoint(self) -> VertexView<&'a mut M, G>
    where
        G: EdgeMidpoint,
        G::Vertex: AsPosition,
    {
        let mut geometry = self.source_vertex().geometry;
        let midpoint = self.midpoint();
        self.split_with(move || {
            *geometry.as_position_mut() = midpoint;
            geometry
        })
    }

    // TODO: What if an edge in the bridging quadrilateral is collapsed, such
    //       as bridging arcs within a triangular ring? Document these
    //       edge cases (no pun intended).
    /// Connects a boundary arc to another boundary arc with a face.
    ///
    /// Bridging arcs inserts a new face and, as needed, new arcs and edges.
    /// The inserted face is always a quadrilateral. The bridged arcs must be
    /// boundary arcs with an orientation that allows them to form a ring.
    ///
    /// Bridging two compatible arcs $\overrightarrow{AB}$ and
    /// $\overrightarrow{CD}$ will result in a ring $\overrightarrow{\\{A,B,
    /// C,D\\}}$.
    ///
    /// Arcs can be bridged within a ring. The destination arc can be chosen by
    /// key or index, where an index selects the $n^\text{th}$ arc from the
    /// source arc within the ring.
    ///
    /// Returns the inserted face if successful.
    ///
    /// # Errors
    ///
    /// Returns an error if the destination arc cannot be found, either arc is
    /// not a boundary arc, or the orientation of the destination arc is
    /// incompatible with the initiating arc.
    ///
    /// # Examples
    ///
    /// Bridging two disjoint quadrilaterals together:
    ///
    /// ```rust
    /// # extern crate nalgebra;
    /// # extern crate plexus;
    /// #
    /// use nalgebra::Point2;
    /// use plexus::graph::{GraphGeometry, MeshGraph, VertexKey, VertexView};
    /// use plexus::prelude::*;
    /// use plexus::primitive::NGon;
    /// use plexus::IntoGeometry;
    ///
    /// fn find<'a, I, T, G>(input: I, geometry: T) -> Option<VertexKey>
    /// where
    ///     I: IntoIterator<Item = VertexView<&'a MeshGraph<G>, G>>,
    ///     T: Copy + IntoGeometry<G::Vertex>,
    ///     G: 'a + GraphGeometry,
    ///     G::Vertex: PartialEq,
    /// {
    ///     input
    ///         .into_iter()
    ///         .find(|vertex| vertex.geometry == geometry.into_geometry())
    ///         .map(|vertex| vertex.key())
    /// }
    ///
    /// let mut graph = MeshGraph::<Point2<f64>>::from_raw_buffers(
    ///     vec![NGon([0usize, 1, 2, 3]), NGon([4, 5, 6, 7])],
    ///     vec![
    ///         (-2.0, 0.0),
    ///         (-1.0, 0.0), // b
    ///         (-1.0, 1.0), // a
    ///         (-2.0, 1.0),
    ///         (1.0, 0.0), // c
    ///         (2.0, 0.0),
    ///         (2.0, 1.0),
    ///         (1.0, 1.0), // d
    ///     ],
    /// )
    /// .unwrap();
    /// let a = find(graph.vertices(), (-1.0, 1.0)).unwrap();
    /// let b = find(graph.vertices(), (-1.0, 0.0)).unwrap();
    /// let c = find(graph.vertices(), (1.0, 0.0)).unwrap();
    /// let d = find(graph.vertices(), (1.0, 1.0)).unwrap();
    /// let face = graph
    ///     .arc_mut((a, b).into())
    ///     .unwrap()
    ///     .bridge(ByKey((c, d).into()))
    ///     .unwrap();
    /// ```
    pub fn bridge(
        self,
        destination: Selector<ArcKey>,
    ) -> Result<FaceView<&'a mut M, G>, GraphError> {
        let destination = destination.key_or_else(|index| {
            self.ring()
                .interior_arcs()
                .nth(index)
                .ok_or_else(|| GraphError::TopologyNotFound)
                .map(|arc| arc.key())
        })?;
        let (storage, source) = self.into_inner().unbind();
        // Errors can easily be caused by inputs to this function. Allow errors
        // from the snapshot to propagate.
        let cache = ArcBridgeCache::snapshot(&storage, source, destination)?;
        Ok(Mutation::replace(storage, Default::default())
            .commit_with(move |mutation| edge::bridge_with_cache(mutation, cache))
            .map(|(storage, face)| View::bind_into(storage, face).expect_consistent())
            .expect_consistent())
    }

    /// Extrudes a boundary arc along its normal into a composite edge.
    ///
    /// Extrusion inserts a new composite edge with the same geometry as the
    /// initiating arc and its composite edge, but modifies the positional
    /// geometry of the new edge's vertices such that they extend geometrically
    /// along the normal of the originating arc. The originating arc is then
    /// bridged with an arc in the opposing edge. This inserts a quadrilateral
    /// face. See `bridge`.
    ///
    /// An arc's normal is perpendicular to the arc and also coplanar with the
    /// arc and one of its neighbors. This is computed via a projection and
    /// supports both 2D and 3D geometries.
    ///
    /// Returns the opposing arc. This is the arc in the destination edge that
    /// is within the same ring as the initiating arc.
    ///
    /// # Errors
    ///
    /// Returns an error if the arc is not a boundary arc.
    ///
    /// # Examples
    ///
    /// Extrude an exterior arc of a quadrilateral.
    ///
    /// ```rust
    /// # extern crate nalgebra;
    /// # extern crate plexus;
    /// #
    /// use nalgebra::Point2;
    /// use plexus::graph::MeshGraph;
    /// use plexus::prelude::*;
    ///
    /// let mut graph = MeshGraph::<Point2<f64>>::from_raw_buffers_with_arity(
    ///     vec![0usize, 1, 2, 3],
    ///     vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
    ///     4,
    /// )
    /// .unwrap();
    /// let key = graph
    ///     .arcs()
    ///     .find(|arc| arc.is_boundary_arc())
    ///     .map(|arc| arc.key())
    ///     .unwrap();
    /// graph.arc_mut(key).unwrap().extrude(1.0).unwrap();
    /// ```
    pub fn extrude<T>(self, offset: T) -> Result<ArcView<&'a mut M, G>, GraphError>
    where
        T: Into<Scalar<VertexPosition<G>>>,
        G: ArcNormal,
        G::Vertex: AsPosition,
        VertexPosition<G>: EuclideanSpace,
    {
        let translation = self.normal() * offset.into();
        let (storage, ab) = self.into_inner().unbind();
        let cache = ArcExtrudeCache::snapshot(&storage, ab, translation).expect_consistent();
        Ok(Mutation::replace(storage, Default::default())
            .commit_with(move |mutation| edge::extrude_with_cache(mutation, cache))
            .map(|(storage, arc)| View::bind_into(storage, arc).expect_consistent())
            .expect_consistent())
    }

    /// Removes the arc and its composite edge.
    ///
    /// Any and all dependent topology is also removed, such as connected
    /// faces, disjoint vertices, etc.
    ///
    /// Returns the source vertex of the initiating arc or `None` if that
    /// vertex becomes disjoint and is also removed. If an arc
    /// $\overrightarrow{AB}$ is removed and its source vertex is not disjoint,
    /// then $A$ is returned.
    pub fn remove(self) -> Option<VertexView<&'a mut M, G>> {
        let a = self.source_vertex().key();
        let (storage, ab) = self.into_inner().unbind();
        let cache = EdgeRemoveCache::snapshot(&storage, ab).expect_consistent();
        Mutation::replace(storage, Default::default())
            .commit_with(move |mutation| edge::remove_with_cache(mutation, cache))
            .map(|(storage, _)| View::bind_into(storage, a))
            .expect_consistent()
    }
}

impl<M, G> Binding for ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>>,
    G: GraphGeometry,
{
    type Key = ArcKey;
    type Payload = Arc<G>;

    /// Gets the key for the arc.
    fn key(&self) -> Self::Key {
        self.inner.key()
    }
}

impl<M, G> Clone for ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>>,
    G: GraphGeometry,
    View<M, Arc<G>>: Clone,
{
    fn clone(&self) -> Self {
        ArcView {
            inner: self.inner.clone(),
        }
    }
}

impl<M, G> Edgoid<M, G> for ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + AsStorage<Edge<G>> + Consistent,
    G: GraphGeometry,
{
    fn into_arc(self) -> ArcView<M, G> {
        self
    }

    fn into_edge(self) -> EdgeView<M, G> {
        self.into_edge()
    }
}

impl<M, G> Copy for ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>>,
    G: GraphGeometry,
    View<M, Arc<G>>: Copy,
{
}

impl<M, G> Deref for ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>>,
    G: GraphGeometry,
{
    type Target = Arc<G>;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl<M, G> DerefMut for ArcView<M, G>
where
    M: ReborrowMut,
    M::Target: AsStorageMut<Arc<G>>,
    G: GraphGeometry,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.deref_mut()
    }
}

impl<M, G> From<View<M, Arc<G>>> for ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>>,
    G: GraphGeometry,
{
    fn from(view: View<M, Arc<G>>) -> Self {
        ArcView { inner: view }
    }
}

impl<M, G> Into<View<M, Arc<G>>> for ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>>,
    G: GraphGeometry,
{
    fn into(self) -> View<M, Arc<G>> {
        let ArcView { inner, .. } = self;
        inner
    }
}

impl<M, G> PartialEq for ArcView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + Consistent,
    G: GraphGeometry,
{
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

/// Orphan view of an arc.
///
/// Provides mutable access to an arc's geometry. See the module documentation
/// for more information about topological views.
pub struct ArcOrphan<'a, G>
where
    G: 'a + GraphGeometry,
{
    inner: Orphan<'a, Arc<G>>,
}

impl<'a, G> Binding for ArcOrphan<'a, G>
where
    G: 'a + GraphGeometry,
{
    type Key = ArcKey;
    type Payload = Arc<G>;

    fn key(&self) -> Self::Key {
        self.inner.key()
    }
}

impl<'a, G> Deref for ArcOrphan<'a, G>
where
    G: 'a + GraphGeometry,
{
    type Target = Arc<G>;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl<'a, G> DerefMut for ArcOrphan<'a, G>
where
    G: 'a + GraphGeometry,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.deref_mut()
    }
}

impl<'a, M, G> From<ArcView<&'a mut M, G>> for ArcOrphan<'a, G>
where
    M: AsStorageMut<Arc<G>>,
    G: 'a + GraphGeometry,
{
    fn from(arc: ArcView<&'a mut M, G>) -> Self {
        Orphan::from(arc.into_inner()).into()
    }
}

impl<'a, G> From<Orphan<'a, Arc<G>>> for ArcOrphan<'a, G>
where
    G: 'a + GraphGeometry,
{
    fn from(inner: Orphan<'a, Arc<G>>) -> Self {
        ArcOrphan { inner }
    }
}

/// View of an edge in a graph.
///
/// Provides traversals, queries, and mutations related to edges in a graph.
/// See the module documentation for more information about topological views.
///
/// An edge connecting a vertex $A$ and a vertex $B$ is notated
/// $\overleftrightarrow{AB}$ or $\overleftrightarrow{BA}$ (both representing
/// the same edge). Typically, edges are described by one of their arcs (e.g.,
/// "the edge of $\overrightarrow{AB}$").
pub struct EdgeView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Edge<G>>,
    G: GraphGeometry,
{
    inner: View<M, Edge<G>>,
}

impl<M, G> EdgeView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Edge<G>>,
    G: GraphGeometry,
{
    fn into_inner(self) -> View<M, Edge<G>> {
        self.into()
    }

    fn interior_reborrow(&self) -> EdgeView<&M::Target, G> {
        self.inner.interior_reborrow().into()
    }
}

impl<'a, M, G> EdgeView<&'a mut M, G>
where
    M: 'a + AsStorageMut<Edge<G>>,
    G: 'a + GraphGeometry,
{
    /// Converts a mutable view into an immutable view.
    ///
    /// This is useful when mutations are not (or no longer) needed and mutual
    /// access is desired.
    pub fn into_ref(self) -> EdgeView<&'a M, G> {
        self.into_inner().into_ref().into()
    }
}

/// Reachable API.
impl<M, G> EdgeView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + AsStorage<Edge<G>>,
    G: GraphGeometry,
{
    pub(in crate::graph) fn into_reachable_arc(self) -> Option<ArcView<M, G>> {
        let key = self.arc;
        self.into_inner().rebind_into(key)
    }

    pub(in crate::graph) fn reachable_arc(&self) -> Option<ArcView<&M::Target, G>> {
        let key = self.arc;
        self.inner.interior_reborrow().rebind_into(key)
    }
}

impl<M, G> EdgeView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + AsStorage<Edge<G>> + Consistent,
    G: GraphGeometry,
{
    pub fn into_arc(self) -> ArcView<M, G> {
        self.into_reachable_arc().expect_consistent()
    }

    pub fn arc(&self) -> ArcView<&M::Target, G> {
        self.reachable_arc().expect_consistent()
    }

    pub fn is_boundary_edge(&self) -> bool {
        let arc = self.arc();
        arc.is_boundary_arc() || arc.opposite_arc().is_boundary_arc()
    }
}

impl<M, G> EdgeView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + AsStorage<Edge<G>> + AsStorage<Vertex<G>> + Consistent,
    G: GraphGeometry,
{
    pub fn midpoint(&self) -> VertexPosition<G>
    where
        G: EdgeMidpoint,
        G::Vertex: AsPosition,
    {
        G::midpoint(self.interior_reborrow()).expect_consistent()
    }
}

impl<M, G> Clone for EdgeView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Edge<G>>,
    G: GraphGeometry,
    View<M, Edge<G>>: Clone,
{
    fn clone(&self) -> Self {
        EdgeView {
            inner: self.inner.clone(),
        }
    }
}

impl<M, G> Edgoid<M, G> for EdgeView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + AsStorage<Edge<G>> + Consistent,
    G: GraphGeometry,
{
    fn into_arc(self) -> ArcView<M, G> {
        self.into_arc()
    }

    fn into_edge(self) -> EdgeView<M, G> {
        self
    }
}

impl<M, G> Copy for EdgeView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Edge<G>>,
    G: GraphGeometry,
    View<M, Edge<G>>: Copy,
{
}

impl<M, G> Deref for EdgeView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Edge<G>>,
    G: GraphGeometry,
{
    type Target = Edge<G>;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl<M, G> DerefMut for EdgeView<M, G>
where
    M: ReborrowMut,
    M::Target: AsStorageMut<Edge<G>>,
    G: GraphGeometry,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.deref_mut()
    }
}

impl<M, G> Binding for EdgeView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Edge<G>>,
    G: GraphGeometry,
{
    type Key = EdgeKey;
    type Payload = Edge<G>;

    /// Gets the key for the edge.
    fn key(&self) -> Self::Key {
        self.inner.key()
    }
}

impl<M, G> From<View<M, Edge<G>>> for EdgeView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Edge<G>>,
    G: GraphGeometry,
{
    fn from(view: View<M, Edge<G>>) -> Self {
        EdgeView { inner: view }
    }
}

impl<M, G> Into<View<M, Edge<G>>> for EdgeView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Edge<G>>,
    G: GraphGeometry,
{
    fn into(self) -> View<M, Edge<G>> {
        let EdgeView { inner, .. } = self;
        inner
    }
}

impl<M, G> PartialEq for EdgeView<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Edge<G>> + Consistent,
    G: GraphGeometry,
{
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

/// Orphan view of an edge.
///
/// Provides mutable access to an edge's geometry. See the module documentation
/// for more information about topological views.
pub struct EdgeOrphan<'a, G>
where
    G: 'a + GraphGeometry,
{
    inner: Orphan<'a, Edge<G>>,
}

impl<'a, G> Binding for EdgeOrphan<'a, G>
where
    G: 'a + GraphGeometry,
{
    type Key = EdgeKey;
    type Payload = Edge<G>;

    fn key(&self) -> Self::Key {
        self.inner.key()
    }
}

impl<'a, G> Deref for EdgeOrphan<'a, G>
where
    G: 'a + GraphGeometry,
{
    type Target = Edge<G>;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl<'a, G> DerefMut for EdgeOrphan<'a, G>
where
    G: 'a + GraphGeometry,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.deref_mut()
    }
}

impl<'a, M, G> From<EdgeView<&'a mut M, G>> for EdgeOrphan<'a, G>
where
    M: AsStorageMut<Edge<G>>,
    G: 'a + GraphGeometry,
{
    fn from(edge: EdgeView<&'a mut M, G>) -> Self {
        Orphan::from(edge.into_inner()).into()
    }
}

impl<'a, G> From<Orphan<'a, Edge<G>>> for EdgeOrphan<'a, G>
where
    G: 'a + GraphGeometry,
{
    fn from(inner: Orphan<'a, Edge<G>>) -> Self {
        EdgeOrphan { inner }
    }
}

pub struct VertexCirculator<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Vertex<G>>,
    G: GraphGeometry,
{
    storage: M,
    inner: <ArrayVec<[VertexKey; 2]> as IntoIterator>::IntoIter,
    phantom: PhantomData<G>,
}

impl<M, G> VertexCirculator<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Vertex<G>>,
    G: GraphGeometry,
{
    fn next(&mut self) -> Option<VertexKey> {
        self.inner.next()
    }
}

impl<M, G> From<ArcView<M, G>> for VertexCirculator<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + AsStorage<Vertex<G>>,
    G: GraphGeometry,
{
    fn from(arc: ArcView<M, G>) -> Self {
        let (a, b) = arc.key().into();
        let (storage, _) = arc.into_inner().unbind();
        VertexCirculator {
            storage,
            inner: ArrayVec::<_>::from([a, b]).into_iter(),
            phantom: PhantomData,
        }
    }
}

impl<M, G> Clone for VertexCirculator<M, G>
where
    M: Clone + Reborrow,
    M::Target: AsStorage<Vertex<G>>,
    G: GraphGeometry,
{
    fn clone(&self) -> Self {
        VertexCirculator {
            storage: self.storage.clone(),
            inner: self.inner.clone(),
            phantom: PhantomData,
        }
    }
}

impl<M, G> ExactSizeIterator for VertexCirculator<M, G>
where
    Self: Iterator,
    M: Reborrow,
    M::Target: AsStorage<Vertex<G>>,
    G: GraphGeometry,
{
}

impl<'a, M, G> Iterator for VertexCirculator<&'a M, G>
where
    M: 'a + AsStorage<Vertex<G>>,
    G: 'a + GraphGeometry,
{
    type Item = VertexView<&'a M, G>;

    fn next(&mut self) -> Option<Self::Item> {
        VertexCirculator::next(self).and_then(|key| View::bind_into(self.storage, key))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.inner.len()))
    }
}

impl<'a, M, G> Iterator for VertexCirculator<&'a mut M, G>
where
    M: 'a + AsStorageMut<Vertex<G>>,
    G: 'a + GraphGeometry,
{
    type Item = VertexOrphan<'a, G>;

    fn next(&mut self) -> Option<Self::Item> {
        VertexCirculator::next(self).and_then(|key| {
            let storage = unsafe {
                mem::transmute::<&'_ mut StorageProxy<Vertex<G>>, &'a mut StorageProxy<Vertex<G>>>(
                    self.storage.as_storage_mut(),
                )
            };
            Orphan::bind_into(storage, key)
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.inner.len()))
    }
}

pub struct FaceCirculator<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Face<G>>,
    G: GraphGeometry,
{
    storage: M,
    inner: <ArrayVec<[FaceKey; 2]> as IntoIterator>::IntoIter,
    phantom: PhantomData<G>,
}

impl<M, G> FaceCirculator<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Face<G>>,
    G: GraphGeometry,
{
    fn next(&mut self) -> Option<FaceKey> {
        self.inner.next()
    }
}

impl<M, G> Clone for FaceCirculator<M, G>
where
    M: Clone + Reborrow,
    M::Target: AsStorage<Face<G>>,
    G: GraphGeometry,
{
    fn clone(&self) -> Self {
        FaceCirculator {
            storage: self.storage.clone(),
            inner: self.inner.clone(),
            phantom: PhantomData,
        }
    }
}

impl<M, G> ExactSizeIterator for FaceCirculator<M, G>
where
    Self: Iterator,
    M: Reborrow,
    M::Target: AsStorage<Face<G>>,
    G: GraphGeometry,
{
}

impl<M, G> From<ArcView<M, G>> for FaceCirculator<M, G>
where
    M: Reborrow,
    M::Target: AsStorage<Arc<G>> + AsStorage<Face<G>>,
    G: GraphGeometry,
{
    fn from(arc: ArcView<M, G>) -> Self {
        let inner = arc
            .face
            .into_iter()
            .chain(
                arc.reachable_opposite_arc()
                    .and_then(|opposite| opposite.face)
                    .into_iter(),
            )
            .collect::<ArrayVec<_>>()
            .into_iter();
        let (storage, _) = arc.into_inner().unbind();
        FaceCirculator {
            storage,
            inner,
            phantom: PhantomData,
        }
    }
}

impl<'a, M, G> Iterator for FaceCirculator<&'a M, G>
where
    M: 'a + AsStorage<Face<G>>,
    G: 'a + GraphGeometry,
{
    type Item = FaceView<&'a M, G>;

    fn next(&mut self) -> Option<Self::Item> {
        FaceCirculator::next(self).and_then(|key| View::bind_into(self.storage, key))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.inner.len()))
    }
}

impl<'a, M, G> Iterator for FaceCirculator<&'a mut M, G>
where
    M: 'a + AsStorageMut<Face<G>>,
    G: 'a + GraphGeometry,
{
    type Item = FaceOrphan<'a, G>;

    fn next(&mut self) -> Option<Self::Item> {
        FaceCirculator::next(self).and_then(|key| {
            let storage = unsafe {
                mem::transmute::<&'_ mut StorageProxy<Face<G>>, &'a mut StorageProxy<Face<G>>>(
                    self.storage.as_storage_mut(),
                )
            };
            Orphan::bind_into(storage, key)
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.inner.len()))
    }
}

#[cfg(test)]
mod tests {
    use decorum::N64;
    use nalgebra::{Point2, Point3};

    use crate::graph::{ArcKey, GraphGeometry, MeshGraph, VertexView};
    use crate::index::HashIndexer;
    use crate::prelude::*;
    use crate::primitive::cube::Cube;
    use crate::primitive::generate::Position;
    use crate::primitive::Tetragon;
    use crate::IntoGeometry;

    fn find_arc_with_vertex_geometry<G, T>(graph: &MeshGraph<G>, geometry: (T, T)) -> Option<ArcKey>
    where
        G: GraphGeometry,
        G::Vertex: PartialEq,
        T: IntoGeometry<G::Vertex>,
    {
        fn find_vertex_with_geometry<G, T>(
            graph: &MeshGraph<G>,
            geometry: T,
        ) -> Option<VertexView<&MeshGraph<G>, G>>
        where
            G: GraphGeometry,
            G::Vertex: PartialEq,
            T: IntoGeometry<G::Vertex>,
        {
            let geometry = geometry.into_geometry();
            graph.vertices().find(|vertex| vertex.geometry == geometry)
        }

        let (source, destination) = geometry;
        find_vertex_with_geometry(graph, source)
            .and_then(|source| {
                find_vertex_with_geometry(graph, destination)
                    .map(move |destination| (source, destination))
            })
            .and_then(|(source, destination)| {
                destination
                    .incoming_arcs()
                    .find(|arc| arc.source_vertex().key() == source.key())
                    .map(|arc| arc.key())
            })
    }

    #[test]
    fn extrude_arc() {
        let mut graph = MeshGraph::<Point2<f32>>::from_raw_buffers_with_arity(
            vec![0u32, 1, 2, 3],
            vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
            4,
        )
        .unwrap();
        let source = find_arc_with_vertex_geometry(&graph, ((1.0, 1.0), (1.0, 0.0))).unwrap();
        graph.arc_mut(source).unwrap().extrude(1.0).unwrap();

        assert_eq!(14, graph.arc_count());
        assert_eq!(2, graph.face_count());
    }

    #[test]
    fn bridge_arcs() {
        // Construct a mesh with two disjoint quadrilaterals.
        let mut graph = MeshGraph::<Point3<f32>>::from_raw_buffers_with_arity(
            vec![0u32, 1, 2, 3, 4, 5, 6, 7],
            vec![
                (-2.0, 0.0, 0.0),
                (-1.0, 0.0, 0.0), // 1
                (-1.0, 1.0, 0.0), // 2
                (-2.0, 1.0, 0.0),
                (1.0, 0.0, 0.0), // 4
                (2.0, 0.0, 0.0),
                (2.0, 1.0, 0.0),
                (1.0, 1.0, 0.0), // 7
            ],
            4,
        )
        .unwrap();
        let source =
            find_arc_with_vertex_geometry(&graph, ((-1.0, 1.0, 0.0), (-1.0, 0.0, 0.0))).unwrap();
        let destination =
            find_arc_with_vertex_geometry(&graph, ((1.0, 0.0, 0.0), (1.0, 1.0, 0.0))).unwrap();
        graph
            .arc_mut(source)
            .unwrap()
            .bridge(ByKey(destination))
            .unwrap();

        assert_eq!(20, graph.arc_count());
        assert_eq!(3, graph.face_count());
    }

    #[test]
    fn split_edge() {
        let (indices, vertices) = Cube::new()
            .polygons::<Position<Point3<N64>>>() // 6 quadrilaterals, 24 vertices.
            .index_vertices::<Tetragon<usize>, _>(HashIndexer::default());
        let mut graph = MeshGraph::<Point3<f64>>::from_raw_buffers(indices, vertices).unwrap();
        let key = graph.arcs().nth(0).unwrap().key();
        let vertex = graph.arc_mut(key).unwrap().split_at_midpoint().into_ref();

        assert_eq!(5, vertex.into_outgoing_arc().into_face().unwrap().arity());
        assert_eq!(
            5,
            vertex
                .into_outgoing_arc()
                .into_opposite_arc()
                .into_face()
                .unwrap()
                .arity()
        );
    }

    #[test]
    fn remove_edge() {
        // Construct a graph with two connected quadrilaterals.
        let mut graph = MeshGraph::<Point2<f32>>::from_raw_buffers_with_arity(
            vec![0u32, 1, 2, 3, 0, 3, 4, 5],
            vec![
                (0.0, 0.0),  // 0
                (1.0, 0.0),  // 1
                (1.0, 1.0),  // 2
                (0.0, 1.0),  // 3
                (-1.0, 1.0), // 4
                (-1.0, 0.0), // 5
            ],
            4,
        )
        .unwrap();

        // The graph should begin with 2 faces.
        assert_eq!(2, graph.face_count());

        // Remove the edge joining the quadrilaterals from the graph.
        let ab = find_arc_with_vertex_geometry(&graph, ((0.0, 0.0), (0.0, 1.0))).unwrap();
        {
            let vertex = graph.arc_mut(ab).unwrap().remove().unwrap().into_ref();

            // The ring should be formed from 6 edges.
            assert_eq!(6, vertex.into_outgoing_arc().into_ring().arity());
        }

        // After the removal, the graph should have no faces.
        assert_eq!(0, graph.face_count());
    }
}
