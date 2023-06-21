// lib.rs
//----------------------------------------------------------------------------------------------------------------------

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

/// panic unless usize is at least 64-bits
pub fn ensure_minimum_usize() {
    // you must be at least 8-bytes tall to ride this ride
    if std::mem::size_of::<usize>() < 8 {
        panic!("usize < 64 bits")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}

//----------------------------------------------------------------------------------------------------------------------
// lib.rs
