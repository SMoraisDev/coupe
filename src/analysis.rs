//! This modules contains functions to help evaluate the quality
//! of the results generated by the partitioning algorithms

use itertools::Itertools;
use snowflake::ProcessUniqueId;

use geometry::{Mbr, PointND};

use nalgebra::allocator::Allocator;
use nalgebra::base::dimension::{DimDiff, DimSub};
use nalgebra::DefaultAllocator;
use nalgebra::DimName;
use nalgebra::U1;

/// Computes the aspect_ratios of several partitions.
///
/// Returns an array of `(ProcessUniqueId, f64)` which associates
/// each partition id to an aspect ratio.
///
/// The size of the returned vector is equal to the number of different
/// ids contained in `partition_id`.
pub fn aspect_ratios<D>(
    partition_ids: &[ProcessUniqueId],
    points: &[PointND<D>],
) -> Vec<(ProcessUniqueId, f64)>
where
    D: DimName + DimSub<U1>,
    DefaultAllocator: Allocator<f64, D>
        + Allocator<f64, D, D>
        + Allocator<f64, U1, D>
        + Allocator<f64, DimDiff<D, U1>>,
    <DefaultAllocator as Allocator<f64, D>>::Buffer: Send + Sync,
    <DefaultAllocator as Allocator<f64, D, D>>::Buffer: Send + Sync,
{
    // Extract each unique partition id from the inpu vector
    let possible_ids = partition_ids.iter().unique();

    // Construct a mapping from each unique partition id
    // to an array containing the points that are contained in that partition
    let id_map: Vec<(ProcessUniqueId, Vec<PointND<D>>)> = possible_ids
        .map(|id| {
            (
                *id,
                partition_ids
                    .iter()
                    .zip(points)
                    .filter(|(id_local, _)| *id == **id_local)
                    .map(|(_, p)| p.clone())
                    .collect(),
            )
        }).collect();

    // for each unique id, turn the constructed
    // array of points into its aspect ratio
    id_map
        .into_iter()
        .map(|(id, points)| (id, Mbr::from_points(&points).aspect_ratio()))
        .collect()
}

/// Computes the weight of each part of a partition
pub fn weights(weights: &[f64], partition: &[ProcessUniqueId]) -> Vec<(ProcessUniqueId, f64)> {
    partition
        .iter()
        .cloned()
        .zip(weights.iter().cloned())
        .into_group_map()
        .into_iter()
        .map(|(id, weights)| (id, weights.into_iter().sum::<f64>()))
        .collect()
}

pub fn imbalance_max_diff(weights: &[f64], partition: &[ProcessUniqueId]) -> f64 {
    let parts_weights = self::weights(weights, partition);

    parts_weights
        .iter()
        .flat_map(|(_id, w1)| parts_weights.iter().map(move |(_id, w2)| (w1 - w2).abs()))
        .max_by(|a, b| a.partial_cmp(&b).unwrap())
        // if the partition is empty, then there is the imbalance is null
        .unwrap_or(0.)
}

pub fn imbalance_relative_diff(weights: &[f64], partition: &[ProcessUniqueId]) -> f64 {
    if weights.is_empty() {
        return 0.;
    }

    let total_weight = weights.iter().sum::<f64>();
    let max_diff = imbalance_max_diff(weights, partition);

    max_diff / total_weight
}

#[cfg(test)]
mod tests {
    use super::*;
    use geometry::Point2D;
    #[test]
    fn test_weights() {
        use std::collections::HashMap;
        use std::iter::FromIterator;

        let id_pool: Vec<_> = (0..3).map(|_| ProcessUniqueId::new()).collect();
        let weights = vec![1., 2., 3., 2., 1.];
        let ids = vec![id_pool[0], id_pool[2], id_pool[0], id_pool[1], id_pool[0]];

        let part_weights = super::weights(&weights, &ids);

        let map = HashMap::<ProcessUniqueId, f64>::from_iter(part_weights.into_iter());

        assert_ulps_eq!(map[&id_pool[0]], 5.);
        assert_ulps_eq!(map[&id_pool[1]], 2.);
        assert_ulps_eq!(map[&id_pool[2]], 2.);
    }

    #[test]
    fn test_imbalance_max_diff() {
        let id_pool: Vec<_> = (0..3).map(|_| ProcessUniqueId::new()).collect();
        let weights = vec![1., 2., 3., 2., 1.];
        let ids = vec![id_pool[0], id_pool[2], id_pool[0], id_pool[1], id_pool[0]];

        let max_diff = imbalance_max_diff(&weights, &ids);
        assert_ulps_eq!(max_diff, 3.);
    }

    #[test]
    fn test_aspect_ratios() {
        let id1 = ProcessUniqueId::new();
        let id2 = ProcessUniqueId::new();

        let ids = vec![id1, id1, id1, id1, id2, id2, id2, id2];
        let points = vec![
            // first rectangle
            Point2D::new(0., 0.),
            Point2D::new(0., 8.),
            Point2D::new(2., 0.),
            Point2D::new(2., 8.),
            // second rectangle
            Point2D::new(-1., 1.),
            Point2D::new(1., -1.),
            Point2D::new(1., 1.),
            Point2D::new(-1., -1.),
        ];

        let ratios = aspect_ratios(&ids, &points);

        for (id, ratio) in ratios {
            match id {
                id if id == id1 => assert_ulps_eq!(ratio, 4.),
                id if id == id2 => assert_ulps_eq!(ratio, 1.),
                _ => unreachable!(),
            }
        }
    }
}
