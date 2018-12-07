//! A mesh partitioning library that implements multithreaded, composable geometric algorithms.
//!
//! # Crate Layout
//!
//! Coupe exposes each algorithm with a struct that implements a trait. There are currently two traits available:
//!
//! - [`InitialPartition`] represents an algorithm that will generate a partition given a set of geometric points and weights.
//! - [`ImprovePartition`] represents an algorithm that will improve an existing partition (previously generated with a [`InitialPartition`]).
//!
//! # Available algorithms
//!
//! ## Initial partitioning algorithms
//! - Space filling curves:
//!   + [`Z-curve`]
//!   + [`Hilbert curve`]
//! - [`Rcb`]: Recursive Coordinate Bisection
//! - [`Rib`]: Recursive Inertial Bisection
//! - [`Multi jagged`]
//!
//! ## Partition improving algorithms
//! - [`KMeans`]
//!
//! [`InitialPartition`]: trait.InitialPartition.html
//! [`ImprovePartition`]: trait.ImprovePartition.html
//! [`Z-curve`]: struct.ZCurve.html
//! [`Hilbert curve`]: struct.HilbertCurve.html
//! [`Rcb`]: struct.Rcb.html
//! [`Rib`]: struct.Rib.html
//! [`Multi jagged`]: struct.MultiJagged.html
//! [`KMeans`]: struct.KMeans.html

#[cfg(test)]
#[macro_use]
extern crate approx;
#[cfg(not(test))]
extern crate approx;
extern crate itertools;
extern crate nalgebra;
extern crate rayon;
extern crate snowflake;

pub mod algorithms;
pub mod analysis;
pub mod geometry;

#[cfg(test)]
mod tests;

// API

// SUBMODULES REEXPORT
pub use geometry::{Point2D, Point3D, PointND};
pub use snowflake::ProcessUniqueId;

pub mod dimension {
    pub use nalgebra::base::dimension::*;
}

use nalgebra::allocator::Allocator;
use nalgebra::base::dimension::{DimDiff, DimSub};
use nalgebra::DefaultAllocator;
use nalgebra::DimName;
use nalgebra::U1;

use std::marker::PhantomData;

pub trait InitialPartition<D>
where
    D: DimName,
    DefaultAllocator: Allocator<f64, D>,
{
    fn partition(&self, points: &[PointND<D>], weights: &[f64]) -> Vec<ProcessUniqueId>;
}

pub trait ImprovePartition<D>
where
    D: DimName,
    DefaultAllocator: Allocator<f64, D>,
{
    fn improve_partition(
        &self,
        points: &[PointND<D>],
        weights: &[f64],
        partition: &mut [ProcessUniqueId],
    );
}

/// # Recursive Coordinate Bisection algorithm
///
/// Partitions a mesh based on the nodes coordinates and coresponding weights.
///
/// This is the most simple and straightforward geometric algorithm. It operates as follows for a N-dimensional set of points:
///
/// At each iteration, select a vector `n` of the canonical basis `(e_0, ..., e_{n-1})`. Then, split the set of points with an hyperplane orthogonal
/// to `n`, such that the two parts of the splits are evenly weighted. Finally, recurse by reapplying the algorithm to the two parts with an other
/// normal vector selection.
///
/// # Example
///
/// ```rust
/// use coupe::Point2D;
/// use coupe::InitialPartition;
///
/// let points = vec![
///     Point2D::new(1., 1.),
///     Point2D::new(-1., 1.),
///     Point2D::new(1., -1.),
///     Point2D::new(-1., -1.),
/// ];
///
/// let weights = vec![1., 1., 1., 1.];
///
/// // generate a partition of 4 parts
/// let rcb = coupe::Rcb { num_iter: 2 };
/// let partition = rcb.partition(&points, &weights);
///
/// for i in 0..4 {
///     for j in 0..4 {
///         if j == i {
///             continue    
///         }
///         assert_ne!(partition[i], partition[j])
///     }
/// }
/// ```
pub struct Rcb {
    pub num_iter: usize,
}

impl<D> InitialPartition<D> for Rcb
where
    D: DimName,
    DefaultAllocator: Allocator<f64, D>,
    <DefaultAllocator as Allocator<f64, D>>::Buffer: Send + Sync,
{
    fn partition(&self, points: &[PointND<D>], weights: &[f64]) -> Vec<ProcessUniqueId> {
        crate::algorithms::recursive_bisection::rcb(points, weights, self.num_iter)
    }
}

/// # Recursive Inertial Bisection algorithm
///
/// Partitions a mesh based on the nodes coordinates and coresponding weights
///
/// This is a variant of the [Rcb](struct.Rcb.html) algorithm, where a basis change is performed beforehand so that
/// the first coordinate of the new basis is colinear to the inertia axis of the set of points. This has the goal
/// of producing better shaped partition than [Rcb](struct.Rcb.html).
///
/// # Example
///
/// ```rust
/// use coupe::Point2D;
/// use coupe::InitialPartition;
///
/// // Here, the inertia axis is the y axis.
/// // We thus expect Rib to split horizontally first.
/// let points = vec![
///     Point2D::new(1., 10.),
///     Point2D::new(-1., 10.),
///     Point2D::new(1., -10.),
///     Point2D::new(-1., -10.),
/// ];
///
/// let weights = vec![1., 1., 1., 1.];
///
/// // generate a partition of 2 parts (1 split)
/// let rib = coupe::Rib { num_iter: 1 };
/// let partition = rib.partition(&points, &weights);
/// eprintln!("partition = {:?}", partition);
///
/// // the two points at the top are in the same partition
/// assert_eq!(partition[0], partition[1]);
///
/// // the two points at the bottom are in the same partition
/// assert_eq!(partition[2], partition[3]);
///
/// // there are two different partition
/// assert_ne!(partition[1], partition[2]);
/// ```
pub struct Rib {
    /// The number of iterations of the algorithm. This will yield a partition of `2^num_iter` parts.
    pub num_iter: usize,
}

impl<D> InitialPartition<D> for Rib
where
    D: DimName + DimSub<U1>,
    DefaultAllocator: Allocator<f64, D, D>
        + Allocator<f64, D>
        + Allocator<f64, U1, D>
        + Allocator<f64, U1, D>
        + Allocator<f64, DimDiff<D, U1>>,
    <DefaultAllocator as Allocator<f64, D>>::Buffer: Send + Sync,
    <DefaultAllocator as Allocator<f64, D, D>>::Buffer: Send + Sync,
{
    fn partition(&self, points: &[PointND<D>], weights: &[f64]) -> Vec<ProcessUniqueId> {
        crate::algorithms::recursive_bisection::rib(points, weights, self.num_iter)
    }
}

/// # Multi-Jagged algorithm
///
/// This algorithm is inspired by Multi-Jagged: A Scalable Parallel Spatial Partitioning Algorithm"
/// by Mehmet Deveci, Sivasankaran Rajamanickam, Karen D. Devine, Umit V. Catalyurek.
///
/// It improves over [RCB](struct.Rcb.html) by following the same idea but by creating more than two subparts
/// in each iteration which leads to decreasing recursion depth. It also allows to generate a partition
/// of any number of parts.
///
/// More precisely, given a number of parts, the algorithm will generate a "partition scheme", which describes how
/// to perform splits at each iteration, such that the total number of iteration is less than `max_iter`.
///
/// More iteration does not necessarily result in a better partition.
///
/// # Example
///
/// ```rust
/// use coupe::Point2D;
/// use coupe::InitialPartition;
///
/// let points = vec![
///     Point2D::new(0., 0.),
///     Point2D::new(1., 0.),
///     Point2D::new(2., 0.),
///     Point2D::new(0., 1.),
///     Point2D::new(1., 1.),
///     Point2D::new(2., 1.),
///     Point2D::new(0., 2.),
///     Point2D::new(1., 2.),
///     Point2D::new(2., 2.),
/// ];
///
/// let weights = vec![1.; 9];
///
/// // generate a partition of 4 parts
/// let multi_jagged = coupe::MultiJagged {
///     num_partitions: 9,
///     max_iter: 4,
/// };
///
/// let partition = multi_jagged.partition(&points, &weights);
///
/// for i in 0..9 {
///     for j in 0..9 {
///         if j == i {
///             continue    
///         }
///         assert_ne!(partition[i], partition[j])
///     }
/// }
/// ```
pub struct MultiJagged {
    pub num_partitions: usize,
    pub max_iter: usize,
}

impl<D> InitialPartition<D> for MultiJagged
where
    D: DimName,
    DefaultAllocator: Allocator<f64, D>,
    <DefaultAllocator as Allocator<f64, D>>::Buffer: Send + Sync,
{
    fn partition(&self, points: &[PointND<D>], weights: &[f64]) -> Vec<ProcessUniqueId> {
        crate::algorithms::multi_jagged::multi_jagged(
            points,
            weights,
            self.num_partitions,
            self.max_iter,
        )
    }
}

/// # Z space-filling curve algorithm
///
/// The Z-curve uses space hashing to partition points. The points in the same part of a partition
/// have the same Z-hash. This hash is computed by recursively constructing a N-dimensional region tree.
///
/// # Example
///
/// ```rust
/// use coupe::Point2D;
/// use coupe::InitialPartition;
///
/// let points = vec![
///     Point2D::new(0., 0.),
///     Point2D::new(1., 1.),
///     Point2D::new(0., 10.),
///     Point2D::new(1., 9.),
///     Point2D::new(9., 1.),
///     Point2D::new(10., 0.),
///     Point2D::new(10., 10.),
///     Point2D::new(9., 9.),
/// ];
///
/// let weights = vec![1.; 8];
///
/// // generate a partition of 4 parts
/// let z_curve = coupe::ZCurve {
///     num_partitions: 4,
///     order: 5,
/// };
///
/// let partition = z_curve.partition(&points, &weights);
///
/// assert_eq!(partition[0], partition[1]);
/// assert_eq!(partition[2], partition[3]);
/// assert_eq!(partition[4], partition[5]);
/// assert_eq!(partition[6], partition[7]);
/// ```  
pub struct ZCurve {
    pub num_partitions: usize,
    pub order: u32,
}

impl<D> InitialPartition<D> for ZCurve
where
    D: DimName + DimSub<U1>,
    DefaultAllocator: Allocator<f64, D, D>
        + Allocator<f64, D>
        + Allocator<f64, U1, D>
        + Allocator<f64, U1, D>
        + Allocator<f64, DimDiff<D, U1>>,
    <DefaultAllocator as Allocator<f64, D>>::Buffer: Send + Sync,
    <DefaultAllocator as Allocator<f64, D, D>>::Buffer: Send + Sync,
{
    fn partition(&self, points: &[PointND<D>], _weights: &[f64]) -> Vec<ProcessUniqueId> {
        crate::algorithms::z_curve::z_curve_partition(points, self.num_partitions, self.order)
    }
}

/// # Hilbert space-filling curve algorithm
///
/// An implementation of the Hilbert curve based on
/// "Encoding and Decoding the Hilbert Order" by XIAN LIU and GÜNTHER SCHRACK.
///
/// This algorithm uses space hashing to reorder points alongside the Hilbert curve ov a giver order.
/// See [wikipedia](https://en.wikipedia.org/wiki/Hilbert_curve) for more details.
///
/// # Example
///
/// ```rust
/// use coupe::Point2D;
/// use coupe::InitialPartition;
///
/// let points = vec![
///     Point2D::new(0., 0.),
///     Point2D::new(1., 1.),
///     Point2D::new(0., 10.),
///     Point2D::new(1., 9.),
///     Point2D::new(9., 1.),
///     Point2D::new(10., 0.),
///     Point2D::new(10., 10.),
///     Point2D::new(9., 9.),
/// ];
///
/// let weights = vec![1.; 8];
///
/// // generate a partition of 4 parts
/// let hilbert = coupe::HilbertCurve {
///     num_partitions: 4,
///     order: 5,
/// };
///
/// let partition = hilbert.partition(&points, &weights);
///
/// assert_eq!(partition[0], partition[1]);
/// assert_eq!(partition[2], partition[3]);
/// assert_eq!(partition[4], partition[5]);
/// assert_eq!(partition[6], partition[7]);
/// ```
pub struct HilbertCurve {
    pub num_partitions: usize,
    pub order: u32,
}

use nalgebra::base::U2;

// hilbert curve is only implemented in 2d for now
impl InitialPartition<U2> for HilbertCurve {
    fn partition(&self, points: &[PointND<U2>], _weights: &[f64]) -> Vec<ProcessUniqueId> {
        crate::algorithms::hilbert_curve::hilbert_curve_partition(
            points,
            _weights,
            self.num_partitions,
            self.order as usize,
        )
    }
}

/// K-means algorithm
///
/// An implementation of the balanced k-means algorithm inspired from
/// "Balanced k-means for Parallel Geometric Partitioning" by Moritz von Looz,
/// Charilaos Tzovas and Henning Meyerhenke (2018, University of Cologne).
///
/// From an initial partition, the K-means algorithm will generate points clusters that will,
/// at each iteration, exchage points with other clusters that are "closer", and move by recomputing the clusters position (defined as
/// the centroid of the points assigned to the cluster). Eventually the clusters will stop moving, yielding a new partition.
///
/// # Example
///
/// ```rust
/// use coupe::Point2D;
/// use coupe::ImprovePartition;
/// use coupe::ProcessUniqueId;
///
/// // create ids for initial partition
/// let p1 = ProcessUniqueId::new();
/// let p2 = ProcessUniqueId::new();
/// let p3 = ProcessUniqueId::new();
///
/// let points = vec![
///     Point2D::new(0., 0.),
///     Point2D::new(1., 0.),
///     Point2D::new(2., 0.),
///     Point2D::new(0., 5.),
///     Point2D::new(1., 5.),
///     Point2D::new(2., 5.),
///     Point2D::new(0., 10.),
///     Point2D::new(1., 10.),
///     Point2D::new(2., 10.),
/// ];
///
/// let weights = vec![1.; 9];
///
/// // create an unbalanced partition:
/// //  - p1: total weight = 1
/// //  - p2: total weight = 7
/// //  - p3: total weight = 1
/// let mut partition = vec![p1, p2, p2, p2, p2, p2, p2, p2, p3];
///
/// let k_means = coupe::KMeans {
///     num_partitions: 3,
///     delta_threshold: 0.,
///     ..Default::default()
/// };
///
/// k_means.improve_partition(&points, &weights, &mut partition);
///
/// assert_eq!(partition[0], partition[1]);
/// assert_eq!(partition[0], partition[2]);
///
/// assert_eq!(partition[3], partition[4]);
/// assert_eq!(partition[3], partition[5]);
///
/// assert_eq!(partition[6], partition[7]);
/// assert_eq!(partition[6], partition[8]);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct KMeans {
    pub num_partitions: usize,
    pub imbalance_tol: f64,
    pub delta_threshold: f64,
    pub max_iter: usize,
    pub max_balance_iter: usize,
    pub erode: bool,
    pub hilbert: bool,
    pub mbr_early_break: bool,
}

impl Default for KMeans {
    fn default() -> Self {
        Self {
            num_partitions: 7,
            imbalance_tol: 5.,
            delta_threshold: 0.01,
            max_iter: 500,
            max_balance_iter: 20, // for now, `max_balance_iter > 1` yields poor convergence time
            erode: false,         // for now, `erode` yields` enabled yields wrong results
            hilbert: true,
            mbr_early_break: false, // for now, `mbr_early_break` enabled yields wrong results
        }
    }
}

impl<D> ImprovePartition<D> for KMeans
where
    D: DimName + DimSub<U1>,
    DefaultAllocator: Allocator<f64, D, D>
        + Allocator<f64, D>
        + Allocator<f64, U1, D>
        + Allocator<f64, U1, D>
        + Allocator<f64, DimDiff<D, U1>>,
    <DefaultAllocator as Allocator<f64, D>>::Buffer: Send + Sync,
    <DefaultAllocator as Allocator<f64, D, D>>::Buffer: Send + Sync,
{
    fn improve_partition(
        &self,
        points: &[PointND<D>],
        weights: &[f64],
        partition: &mut [ProcessUniqueId],
    ) {
        let settings = crate::algorithms::k_means::BalancedKmeansSettings {
            num_partitions: self.num_partitions,
            imbalance_tol: self.imbalance_tol,
            delta_threshold: self.delta_threshold,
            max_iter: self.max_iter,
            max_balance_iter: self.max_balance_iter,
            erode: self.erode,
            hilbert: self.hilbert,
            mbr_early_break: self.mbr_early_break,
        };
        crate::algorithms::k_means::balanced_k_means_with_initial_partition(
            points, weights, settings, partition,
        )
    }
}

pub struct Composition<T, U> {
    first: T,
    second: U,
}

impl<D, T, U> InitialPartition<D> for Composition<T, U>
where
    D: DimName,
    DefaultAllocator: Allocator<f64, D>,
    T: InitialPartition<D>,
    U: ImprovePartition<D>,
{
    fn partition(&self, points: &[PointND<D>], weights: &[f64]) -> Vec<ProcessUniqueId> {
        let mut partition = self.first.partition(points, weights);
        self.second
            .improve_partition(points, weights, &mut partition);
        partition
    }
}

impl<D, T, U> ImprovePartition<D> for Composition<T, U>
where
    D: DimName,
    DefaultAllocator: Allocator<f64, D>,
    T: ImprovePartition<D>,
    U: ImprovePartition<D>,
{
    fn improve_partition(
        &self,
        points: &[PointND<D>],
        weights: &[f64],
        partition: &mut [ProcessUniqueId],
    ) {
        self.first.improve_partition(points, weights, partition);
        self.second.improve_partition(points, weights, partition);
    }
}

pub trait Compose<T> {
    type Composed;
    fn compose<D>(self, other: T) -> Self::Composed
    where
        D: DimName,
        DefaultAllocator: Allocator<f64, D>;
}

impl<T, U> Compose<T> for U {
    type Composed = Composition<Self, T>;
    fn compose<D>(self, other: T) -> Self::Composed {
        Composition {
            first: self,
            second: other,
        }
    }
}
