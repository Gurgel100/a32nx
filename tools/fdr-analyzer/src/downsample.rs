//! Down-sampling algorithms for reducing millions of data points to screen pixels.
//!
//! Implements Largest-Triangle-Three-Buckets (LTTB) which preserves visual shape
//! much better than simple decimation.

use crate::fdr_reader::DataPoint;

/// Apply Largest-Triangle-Three-Buckets downsampling.
///
/// Reduces `input` to at most `max_points` points by selecting the data point
/// in each bucket that maximizes the triangle area formed with the previous and
/// next retained points.
pub fn lttb(input: &[DataPoint], max_points: usize) -> Vec<DataPoint> {
    if input.len() <= max_points {
        return input.to_vec();
    }

    let bucket_size = (input.len() as f64 / max_points as f64) as usize;
    let mut result = Vec::with_capacity(max_points);

    // Always keep the first point.
    result.push(input[0].clone());

    let mut prev_idx = 0usize;

    for bucket_start in (bucket_size..input.len()).step_by(bucket_size) {
        let bucket_end = (bucket_start + bucket_size).min(input.len());

        // Estimate next retained point as the midpoint of the *next* bucket.
        let next_bucket_start = bucket_end.min(input.len());
        let next_bucket_end = (next_bucket_start + bucket_size).min(input.len());
        let next_idx = if next_bucket_end > next_bucket_start {
            (next_bucket_start + next_bucket_end) / 2
        } else {
            next_bucket_start
        };

        let prev = &input[prev_idx];
        let next = &input[next_idx];

        // Find the point in this bucket that forms the largest triangle.
        let mut best_area = -1.0;
        let mut best_idx = bucket_start;

        for i in bucket_start..bucket_end {
            let area = triangle_area(prev, &input[i], next);
            if area > best_area {
                best_area = area;
                best_idx = i;
            }
        }

        result.push(input[best_idx].clone());
        prev_idx = best_idx;
    }

    // Always keep the last point.
    if let Some(last) = input.last() {
        if result.last() != Some(last) {
            result.push(last.clone());
        }
    }

    result
}

/// Area of the triangle formed by three points (absolute value, ignoring sign).
fn triangle_area(a: &DataPoint, b: &DataPoint, c: &DataPoint) -> f64 {
    // | (x_b - x_a)(y_c - y_a) - (x_c - x_a)(y_b - y_a) | / 2
    // The /2 is a constant scaling factor we can skip for comparison purposes.
    let dx1 = b.time_s - a.time_s;
    let dy1 = b.value - a.value;
    let dx2 = c.time_s - a.time_s;
    let dy2 = c.value - a.value;
    (dx1 * dy2 - dx2 * dy1).abs()
}
