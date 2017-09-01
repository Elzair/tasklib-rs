// Return a vector with only Some(T) unwrapped elements.
#[inline]
pub fn filter_vec<T>(v: Vec<Option<T>>) -> Vec<T> {
    v.into_iter().filter_map(|n| { n }).collect::<Vec<_>>()
}

// Split a `Vec` into `n` different Vecs of length `r`.
#[inline]
pub fn split_vec<T>(mut v: Vec<T>, n: usize, r: usize) -> Vec<Vec<T>> {
    assert!(n*r == v.len());

    let mut res = Vec::<Vec<T>>::with_capacity(n);

    #[allow(unused_variables)]
    for nn in 0..n {
        res.push(v.drain(0..r).collect());
    }

    res
}

#[inline]
pub fn is_diagonal(index: usize, row_size: usize) -> bool {
    let (y, x) = get_yx(index, row_size);
    y == x
}

#[inline]
pub fn is_lower_left(index: usize, row_size: usize) -> bool {
    let (y, x) = get_yx(index, row_size);
    y > x
}

#[inline]
pub fn get_transpose_index(index: usize, row_size: usize) -> usize {
    let (y, x) = get_yx(index, row_size);
    x * row_size + y 
}

#[inline]
fn get_yx(index: usize, row_size: usize) -> (usize, usize) {
    (index / row_size, index % row_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_vec() {
        let vec = vec![0, 1, 2,
                       3, 4, 5,
                       6, 7, 8,
                       9, 10, 11];
        let (n, r) = (4, 3);
        let vec2 = split_vec(vec, 4, 3);
        assert_eq!(vec2.len(), n);
        for vec3 in vec2 {
            assert_eq!(vec3.len(), r);
        }
    }
}
