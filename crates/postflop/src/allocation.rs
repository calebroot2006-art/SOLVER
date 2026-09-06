//! Fallible allocation of solver buffers whose lengths come from checked layouts.

use crate::SolveError;

pub(crate) fn reserved<T>(len: usize) -> Result<Vec<T>, SolveError> {
    let mut values = Vec::new();
    values.try_reserve_exact(len).map_err(|error| {
        SolveError::Allocation(format!("cannot reserve {len} entries: {error}"))
    })?;
    Ok(values)
}

pub(crate) fn filled<T: Clone>(len: usize, value: T) -> Result<Vec<T>, SolveError> {
    let mut values = reserved(len)?;
    values.resize(len, value);
    Ok(values)
}

pub(crate) fn collect<T>(iter: impl ExactSizeIterator<Item = T>) -> Result<Vec<T>, SolveError> {
    let mut values = reserved(iter.len())?;
    values.extend(iter);
    Ok(values)
}

pub(crate) fn try_collect<T>(
    iter: impl ExactSizeIterator<Item = Result<T, SolveError>>,
) -> Result<Vec<T>, SolveError> {
    let mut values = reserved(iter.len())?;
    for value in iter {
        values.push(value?);
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impossible_capacity_returns_a_checked_error() {
        assert!(matches!(
            reserved::<f64>(usize::MAX),
            Err(SolveError::Allocation(_))
        ));
    }
}
