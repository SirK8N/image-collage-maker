/// Pure-Rust implementation of the Jonker-Volgenant (LAPJV) algorithm
/// for solving the square Linear Assignment Problem (minimizing sum_i cost[i, rowsol[i]]).
///
/// Reference: R. Jonker and A. Volgenant, "A Shortest Augmenting Path Algorithm
/// for Dense and Sparse Linear Assignment Problems", Computing 38, 325-340 (1987).
pub fn solve_lapjv<F>(cost: &[f32], n: usize, mut on_progress: F) -> (Vec<usize>, f64)
where
    F: FnMut(usize, usize),
{
    if n == 0 {
        return (Vec::new(), 0.0);
    }
    if n == 1 {
        on_progress(1, 1);
        return (vec![0], cost[0] as f64);
    }

    const NONE: usize = usize::MAX;
    let mut rowsol = vec![NONE; n];
    let mut colsol = vec![NONE; n];
    let mut v = vec![0.0f64; n];
    let mut matches = vec![0usize; n];

    // 1. Column reduction
    for j in (0..n).rev() {
        let mut min_val = cost[j] as f64;
        let mut imin = 0usize;
        for i in 1..n {
            let c = cost[i * n + j] as f64;
            if c < min_val {
                min_val = c;
                imin = i;
            }
        }
        v[j] = min_val;
        matches[imin] += 1;
        if matches[imin] == 1 {
            rowsol[imin] = j;
            colsol[j] = imin;
        } else {
            colsol[j] = NONE;
        }
    }

    // 2. Reduction transfer
    let mut free_rows = Vec::with_capacity(n);
    for i in 0..n {
        if matches[i] == 0 {
            free_rows.push(i);
        } else if matches[i] == 1 {
            let j1 = rowsol[i];
            let mut min_val = f64::INFINITY;
            let row_off = i * n;
            for j in 0..n {
                if j != j1 {
                    let red = cost[row_off + j] as f64 - v[j];
                    if red < min_val {
                        min_val = red;
                    }
                }
            }
            v[j1] -= min_val;
        }
    }

    // 3. Augmenting row reduction (2 passes)
    for _pass in 0..2 {
        let mut next_free = Vec::with_capacity(free_rows.len());
        let mut k = 0usize;
        let num_free = free_rows.len();

        while k < num_free {
            let mut i = free_rows[k];
            k += 1;
            let mut umin = cost[i * n] as f64 - v[0];
            let mut j1 = 0usize;
            let mut usubmin = f64::INFINITY;
            let mut j2 = 0usize;

            let row_off = i * n;
            for j in 1..n {
                let h = cost[row_off + j] as f64 - v[j];
                if h < usubmin {
                    if h >= umin {
                        usubmin = h;
                        j2 = j;
                    } else {
                        usubmin = umin;
                        umin = h;
                        j2 = j1;
                        j1 = j;
                    }
                }
            }

            let mut i0 = colsol[j1];
            let mut transfer_steps = 0usize;
            loop {
                if umin < usubmin {
                    v[j1] -= usubmin - umin;
                } else if i0 != NONE {
                    j1 = j2;
                    i0 = colsol[j2];
                }

                rowsol[i] = j1;
                colsol[j1] = i;

                if i0 == NONE {
                    break;
                }

                if umin < usubmin && transfer_steps < n {
                    // Re-process displaced row i0 immediately
                    i = i0;
                    transfer_steps += 1;
                    let row_off = i * n;
                    umin = cost[row_off] as f64 - v[0];
                    j1 = 0;
                    usubmin = f64::INFINITY;
                    j2 = 0;
                    for j in 1..n {
                        let h = cost[row_off + j] as f64 - v[j];
                        if h < usubmin {
                            if h >= umin {
                                usubmin = h;
                                j2 = j;
                            } else {
                                usubmin = umin;
                                umin = h;
                                j2 = j1;
                                j1 = j;
                            }
                        }
                    }
                    i0 = colsol[j1];
                } else {
                    next_free.push(i0);
                    break;
                }
            }
        }

        free_rows = next_free;
    }

    // 4. Augment solution for each remaining unassigned row (Dijkstra shortest augmenting path)
    let total_free = free_rows.len().max(1);
    let mut d = vec![0.0f64; n];
    let mut pred = vec![0usize; n];
    let mut collist: Vec<usize> = (0..n).collect();

    for (step, &freerow) in free_rows.iter().enumerate() {
        let row_off = freerow * n;
        for j in 0..n {
            d[j] = cost[row_off + j] as f64 - v[j];
            pred[j] = freerow;
            collist[j] = j;
        }

        let mut low = 0usize;
        let mut up = 0usize;
        let mut last = 0usize;
        let mut min_val = 0.0f64;
        let mut endofpath = NONE;

        while endofpath == NONE {
            if up == low {
                last = low;
                min_val = d[collist[up]];
                up += 1;
                for k in up..n {
                    let j = collist[k];
                    let h = d[j];
                    if h <= min_val {
                        if h < min_val {
                            up = low;
                            min_val = h;
                        }
                        collist[k] = collist[up];
                        collist[up] = j;
                        up += 1;
                    }
                }
                for k in low..up {
                    if colsol[collist[k]] == NONE {
                        endofpath = collist[k];
                        break;
                    }
                }
            }

            if endofpath == NONE {
                let j1 = collist[low];
                low += 1;
                let i = colsol[j1];
                let i_off = i * n;
                let h = cost[i_off + j1] as f64 - v[j1] - min_val;

                for k in up..n {
                    let j = collist[k];
                    let v2 = cost[i_off + j] as f64 - v[j] - h;
                    if v2 < d[j] {
                        pred[j] = i;
                        if (v2 - min_val).abs() <= 1e-12 || v2 <= min_val {
                            if colsol[j] == NONE {
                                endofpath = j;
                                break;
                            } else {
                                collist[k] = collist[up];
                                collist[up] = j;
                                up += 1;
                            }
                        }
                        d[j] = v2;
                    }
                }
            }
        }

        // Update column dual variables
        for k in 0..last {
            let j1 = collist[k];
            v[j1] += d[j1] - min_val;
        }

        // Augment along the predecessor chain back to freerow
        let mut j = endofpath;
        loop {
            let i = pred[j];
            colsol[j] = i;
            let prev_j = rowsol[i];
            rowsol[i] = j;
            if i == freerow {
                break;
            }
            j = prev_j;
        }

        if step % 16 == 0 || step + 1 == free_rows.len() {
            on_progress(step + 1, total_free);
        }
    }

    if free_rows.is_empty() {
        on_progress(1, 1);
    }

    let mut total_cost = 0.0f64;
    for i in 0..n {
        total_cost += cost[i * n + rowsol[i]] as f64;
    }

    (rowsol, total_cost)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, Rng, SeedableRng};

    fn brute_force_min(cost: &[f32], n: usize) -> f64 {
        fn permute(
            cost: &[f32],
            n: usize,
            row: usize,
            used: &mut [bool],
            acc: f64,
            best: &mut f64,
        ) {
            if row == n {
                if acc < *best {
                    *best = acc;
                }
                return;
            }
            if acc >= *best {
                return;
            }
            for col in 0..n {
                if !used[col] {
                    used[col] = true;
                    permute(cost, n, row + 1, used, acc + cost[row * n + col] as f64, best);
                    used[col] = false;
                }
            }
        }
        let mut used = vec![false; n];
        let mut best = f64::INFINITY;
        permute(cost, n, 0, &mut used, 0.0, &mut best);
        best
    }

    #[test]
    fn test_lapjv_matches_bruteforce_random() {
        let mut rng = StdRng::seed_from_u64(42);
        for n in 1..=8 {
            for _ in 0..10 {
                let cost: Vec<f32> = (0..n * n).map(|_| rng.gen_range(0.0..100.0)).collect();
                let (sol, total) = solve_lapjv(&cost, n, |_, _| {});
                let expected = brute_force_min(&cost, n);
                assert_eq!(sol.len(), n);
                let mut seen = vec![false; n];
                for &c in &sol {
                    assert!(!seen[c], "duplicate assignment");
                    seen[c] = true;
                }
                assert!(
                    (total - expected).abs() < 1e-3,
                    "n={n}: got {total}, expected {expected}"
                );
            }
        }
    }

    #[test]
    fn test_lapjv_with_ties_and_larger_matrix() {
        let mut rng = StdRng::seed_from_u64(1234);
        let n = 64;
        // Discrete values to trigger many ties
        let cost: Vec<f32> = (0..n * n).map(|_| rng.gen_range(0..5) as f32).collect();
        let (sol, _total) = solve_lapjv(&cost, n, |_, _| {});
        assert_eq!(sol.len(), n);
        let mut seen = vec![false; n];
        for &c in &sol {
            assert!(!seen[c]);
            seen[c] = true;
        }
    }
}

