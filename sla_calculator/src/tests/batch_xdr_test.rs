#[cfg(test)]
mod batch_xdr_tests {
    use super::*;
    use soroban_sdk::{Env, Symbol};

    #[test]
    fn test_error_vector_xdr_mapping() {
        let env = Env::default();

        // None mapping: empty vector
        let none_vec = BatchResultAdapter::create_error_vector(&env, None);
        assert_eq!(none_vec.len(), 0);

        // Some mapping: single-element vector
        let err_sym = Symbol::new(&env, "BatchError");
        let some_vec = BatchResultAdapter::create_error_vector(&env, Some(err_sym));
        assert_eq!(some_vec.len(), 1);
        assert_eq!(some_vec.get(0).unwrap(), err_sym);
    }
}