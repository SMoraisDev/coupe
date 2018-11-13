//! An implementation of the Multi-Jagged spatial partitioning
//! inpired by "Multi-Jagged: A Scalable Parallel Spatial Partitioning Algorithm"
//! by Mehmet Deveci, Sivasankaran Rajamanickam, Karen D. Devine, Umit V. Catalyurek
//!
//! It improves over RCB by following the same idea but by creating more than two subparts
//! in each iteration which leads to decreasing recursion depth.

use crate::geometry::*;
use rayon::prelude::*;
use snowflake::ProcessUniqueId;

use std::cmp::Ordering;
use std::sync::atomic::{self, AtomicPtr};

fn is_prime(n: u32) -> bool {
    if n < 2 {
        return false;
    }
    let p: u32 = (f64::from(n)).sqrt() as u32;

    for i in 2..=p {
        if n % i == 0 {
            return false;
        }
    }
    true
}

// Computes the list of primes factors of a given number n
fn prime_factors(mut n: u32) -> Vec<u32> {
    if n <= 1 {
        return vec![1];
    }

    let mut ret = vec![];
    let mut primes = (2..).filter(|n| is_prime(*n));
    let mut current = primes.next().unwrap();
    while n > 1 {
        while n % current == 0 {
            ret.push(current);
            n /= current;
        }
        current = primes.next().unwrap();
    }

    ret
}

// Computes from a set of points, how many sections will be made at each iteration;
fn partition_scheme(_points: &[Point2D], num_parts: usize) -> Vec<usize> {
    // for now the points are ignored
    // TODO: improve by adapting scheme with geometry, e.g. aspect ratio
    let primes = prime_factors(num_parts as u32);

    primes.into_iter().map(|p| p as usize).collect()
}

pub fn multi_jagged_2d_with_scheme(
    points: &[Point2D],
    weights: &[f64],
    partition_scheme: &[usize],
) -> Vec<ProcessUniqueId> {
    let len = points.len();
    let mut permutation = (0..len).into_par_iter().collect::<Vec<_>>();
    let initial_id = ProcessUniqueId::new();
    let mut initial_partition = rayon::iter::repeat(initial_id)
        .take(len)
        .collect::<Vec<_>>();

    multi_jagged_2d_recurse(
        points,
        weights,
        &mut permutation,
        &AtomicPtr::new(initial_partition.as_mut_ptr()),
        true,
        &partition_scheme,
    );

    initial_partition
}

fn multi_jagged_2d_recurse(
    points: &[Point2D],
    weights: &[f64],
    permutation: &mut [usize],
    partition: &AtomicPtr<ProcessUniqueId>,
    x_axis: bool,
    partition_scheme: &[usize],
) {
    if let Some(num_splits) = partition_scheme.iter().next() {
        axis_sort(points, permutation, x_axis);

        let split_positions = compute_split_positions(weights, permutation, *num_splits);
        let mut sub_permutations = split_at_mut_many(permutation, &split_positions);

        let x_axis = !x_axis;
        sub_permutations.par_iter_mut().for_each(|permu| {
            multi_jagged_2d_recurse(
                points,
                weights,
                permu,
                partition,
                x_axis,
                &partition_scheme[1..],
            )
        });
    } else {
        let part_id = ProcessUniqueId::new();
        permutation.par_iter().for_each(|idx| {
            let ptr = partition.load(atomic::Ordering::Relaxed);
            unsafe { std::ptr::write(ptr.add(*idx), part_id) }
        });
    }
}

fn axis_sort(points: &[Point2D], permutation: &mut [usize], x_axis: bool) {
    if x_axis {
        permutation.par_sort_by(|i1, i2| is_less_cmp_f64(points[*i1].x, points[*i2].x));
    } else {
        permutation.par_sort_by(|i1, i2| is_less_cmp_f64(points[*i1].y, points[*i2].y));
    }
}

fn compute_split_positions(
    weights: &[f64],
    permutation: &[usize],
    num_splits: usize,
) -> Vec<usize> {
    let total_weight = permutation.par_iter().map(|idx| weights[*idx]).sum::<f64>();

    let weight_thresholds = (1..=num_splits)
        .map(|n| total_weight * n as f64 / (num_splits + 1) as f64)
        .collect::<Vec<_>>();

    let mut ret = Vec::with_capacity(num_splits);

    let mut scan = permutation
        .par_iter()
        .enumerate()
        .fold_with((std::usize::MAX, 0.), |(low, acc), (idx, val)| {
            if idx < low {
                (idx, acc + weights[*val])
            } else {
                (low, acc + weights[*val])
            }
        }).collect::<Vec<_>>()
        .into_iter();

    let mut current_weights_sum = 0.;
    let mut current_weights_sums_cache = Vec::with_capacity(num_splits);

    for threshold in weight_thresholds.iter() {
        // if this condition is verified, it means that a block of the scan contained more than one threshold
        // and the current threshold was skipped during previous iteration. We just
        // push the last element again and skip the rest of the iteration
        if current_weights_sum > *threshold {
            let last = ret[ret.len() - 1];
            ret.push(last);
            let last = current_weights_sums_cache[current_weights_sums_cache.len() - 1];
            current_weights_sums_cache.push(last);
            continue;
        }

        'inner: loop {
            let current = scan.next().unwrap();
            if current_weights_sum + current.1 > *threshold {
                ret.push(current.0);
                current_weights_sums_cache.push(current_weights_sum);
                current_weights_sum += current.1;
                break 'inner;
            }
            current_weights_sum += current.1;
        }
    }

    ret.into_par_iter()
        .zip(current_weights_sums_cache)
        .zip(weight_thresholds)
        .map(|((mut idx, mut sum), threshold)| {
            while sum < threshold {
                idx += 1;
                sum += weights[permutation[idx]];
            }
            idx
        }).collect()
}

// Same as slice::split_at_mut but split in a arbitrary number of subslices
// Sequential since `position` should be small
fn split_at_mut_many<'a, T>(slice: &'a mut [T], positions: &[usize]) -> Vec<&'a mut [T]> {
    let ret = Vec::with_capacity(positions.len() + 1);

    let (mut head, tail, _) = positions.iter().fold(
        (ret, slice, 0),
        |(mut acc_ret, acc_slice, drained_count), pos| {
            let (sub, next) = acc_slice.split_at_mut(*pos - drained_count);
            let len = sub.len();
            acc_ret.push(sub);
            (acc_ret, next, drained_count + len)
        },
    );

    head.push(tail);
    head
}

fn is_less_cmp_f64(a: f64, b: f64) -> Ordering {
    if a < b {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_is_prime() {
        assert!(!is_prime(0));
        assert!(!is_prime(1));
        assert!(is_prime(2));
        assert!(is_prime(3));
        assert!(!is_prime(4));
        assert!(is_prime(5));
        assert!(!is_prime(6));
        assert!(is_prime(7));
        assert!(!is_prime(8));
        assert!(!is_prime(9));
        assert!(!is_prime(10));
        assert!(is_prime(11));
        assert!(!is_prime(12));
        assert!(is_prime(13));
    }

    #[test]
    fn test_prime_factors() {
        assert_eq!(
            prime_factors(2 * 3 * 3 * 5 * 7 * 11 * 13 * 17),
            vec![2, 3, 3, 5, 7, 11, 13, 17]
        );
    }

    #[test]
    fn test_split_at_mut_many() {
        let array = &mut [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

        let sub_arrays = split_at_mut_many(array, &[1, 3, 6, 9, 11]);

        assert_eq!(
            sub_arrays,
            vec![
                &mut [0][..],
                &mut [1, 2][..],
                &mut [3, 4, 5][..],
                &mut [6, 7, 8][..],
                &mut [9, 10][..],
                &mut [11, 12][..],
            ]
        )
    }
}
