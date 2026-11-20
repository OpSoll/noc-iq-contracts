use soroban_sdk::{symbol_short, Env, Symbol, Vec};

pub const INIT_WRITE_KEYS: [Symbol; 8] = [
    symbol_short!("ADMIN"),
    symbol_short!("OPERATOR"),
    symbol_short!("PAUSED"),
    symbol_short!("STATS"),
    symbol_short!("HIST"),
    symbol_short!("INIT_TIME"),
    symbol_short!("CONFIG"),
    symbol_short!("VER"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitError {
    AlreadyInitialized,
    IncompleteInitialization,
    InjectedFault,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitializationState {
    Uninitialized,
    Partial,
    Complete,
}

/// Guards `initialize()` against being called more than once.
pub fn ensure_not_initialized(env: &Env) -> Result<(), InitError> {
    if initialization_state(env) == InitializationState::Complete {
        return Err(InitError::AlreadyInitialized);
    }
    Ok(())
}

pub fn initialization_state(env: &Env) -> InitializationState {
    let mut written = 0usize;
    for key in INIT_WRITE_KEYS.iter() {
        if env.storage().instance().has(key) {
            written += 1;
        }
    }
    if written == 0 {
        InitializationState::Uninitialized
    } else if written == INIT_WRITE_KEYS.len() {
        InitializationState::Complete
    } else {
        InitializationState::Partial
    }
}

pub fn missing_write_keys(env: &Env) -> Vec<Symbol> {
    let mut missing = Vec::new(env);
    for key in INIT_WRITE_KEYS.iter() {
        if !env.storage().instance().has(key) {
            missing.push_back(key.clone());
        }
    }
    missing
}

pub fn run_initialization<F>(env: &Env, write: F) -> Result<(), InitError>
where
    F: FnMut(&Env, Symbol) -> Result<(), InitError>,
{
    run_initialization_with_fault(env, None, write)
}

pub fn run_initialization_with_fault<F>(
    env: &Env,
    fail_at: Option<usize>,
    mut write: F,
) -> Result<(), InitError>
where
    F: FnMut(&Env, Symbol) -> Result<(), InitError>,
{
    ensure_not_initialized(env)?;
    let missing = missing_write_keys(env);
    for (index, key) in missing.iter().enumerate() {
        if fail_at == Some(index) {
            return Err(InitError::InjectedFault);
        }
        write(env, key.clone())?;
    }
    if initialization_state(env) != InitializationState::Complete {
        return Err(InitError::IncompleteInitialization);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;

    fn write_boundary(env: &Env, key: Symbol) -> Result<(), InitError> {
        env.storage().instance().set(&key, &1u32);
        Ok(())
    }

    #[test]
    fn failure_before_and_after_each_write_boundary_can_be_retried() {
        for fail_at in 0..=INIT_WRITE_KEYS.len() {
            let env = Env::default();
            let id = env.register_contract(None, SLACalculatorContract);
            env.as_contract(&id, || {
                let result = run_initialization_with_fault(&env, Some(fail_at), write_boundary);
                if fail_at == INIT_WRITE_KEYS.len() {
                    assert_eq!(result, Ok(()));
                    assert_eq!(initialization_state(&env), InitializationState::Complete);
                } else {
                    assert_eq!(result, Err(InitError::InjectedFault));
                    assert_ne!(initialization_state(&env), InitializationState::Complete);
                    assert_eq!(run_initialization(&env, write_boundary), Ok(()));
                    assert_eq!(initialization_state(&env), InitializationState::Complete);
                }
                assert_eq!(
                    ensure_not_initialized(&env),
                    Err(InitError::AlreadyInitialized)
                );
            });
        }
    }

    #[test]
    fn partial_state_is_not_reported_as_initialized() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            env.storage().instance().set(&INIT_WRITE_KEYS[0], &1u32);
            assert_eq!(initialization_state(&env), InitializationState::Partial);
            assert_eq!(ensure_not_initialized(&env), Ok(()));
            assert_eq!(missing_write_keys(&env).len(), INIT_WRITE_KEYS.len() - 1);
        });
    }
}
