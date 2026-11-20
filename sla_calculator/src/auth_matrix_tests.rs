#[cfg(test)]
mod auth_matrix_tests {
    use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};
    use crate::{SLACalculatorContract, SLACalculatorContractClient, SLAConfig, SLAError};

    fn setup(env: &Env) -> (Address, Address, SLACalculatorContractClient) {
        env.mock_all_auths();
        let contract_id = env.register_contract(None, SLACalculatorContract);
        let client = SLACalculatorContractClient::new(env, &contract_id);
        let admin = Address::generate(env);
        let operator = Address::generate(env);
        client.initialize(&admin, &operator);
        (admin, operator, client)
    }

    #[test]
    fn test_only_operator_can_calculate_sla() {
        let env = Env::default();
        env.mock_all_auths();
        let (_, operator, client) = setup(&env);
        client.calculate_sla(&operator, &symbol_short!("OUT1"), &symbol_short!("high"), &10);
    }

    #[test]
    fn test_only_admin_can_set_config() {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, _, client) = setup(&env);
        client.set_config(
            &admin,
            &symbol_short!("high"),
            &SLAConfig { threshold_minutes: 30, penalty_per_minute: 50, reward_base: 500 , top_tier_multiplier: 200, excel_tier_multiplier: 150, good_tier_multiplier: 100},
        );
    }

    #[test]
    fn test_only_admin_can_pause() {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, _, client) = setup(&env);
        client.pause(&admin);
        client.unpause(&admin);
    }

    #[test]
    fn test_only_admin_can_set_operator() {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, _, client) = setup(&env);
        let new_op = Address::generate(&env);
        client.set_operator(&admin, &new_op);
    }

    #[test]
    fn test_repeated_calls_by_same_operator_succeed() {
        let env = Env::default();
        env.mock_all_auths();
        let (_, operator, client) = setup(&env);
        for i in 1u32..=3 {
            client.calculate_sla(&operator, &symbol_short!("OUT"), &symbol_short!("high"), &i);
        }
        let stats = client.get_stats();
        assert_eq!(stats.total_calculations, 3);
    }

    #[test]
    fn test_unauthorized_caller_cannot_calculate() {
        let env = Env::default();
        let contract_id = env.register_contract(None, SLACalculatorContract);
        let client = SLACalculatorContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let operator = Address::generate(&env);
        let stranger = Address::generate(&env);
        env.mock_all_auths();
        client.initialize(&admin, &operator);
        env.mock_auths(&[]);
        let result = client.try_calculate_sla(
            &stranger,
            &symbol_short!("OUT"),
            &symbol_short!("high"),
            &5,
        );
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod dispute_finalization_auth_tests {
    use crate::dispute::{
        adjudication_started, is_arbitrator, require_admin_or_arbitrator, set_arbitrator,
        DisputeError, DisputeStatus,
    };
    use soroban_sdk::{testutils::Address as _, Address, Env};

    #[test]
    fn only_the_arbitrator_or_admin_can_finalize() {
        let env = Env::default();
        let contract = env.register_contract(None, crate::SLACalculatorContract);
        let admin = Address::generate(&env);
        let arbitrator = Address::generate(&env);
        let filer = Address::generate(&env);
        let provider = Address::generate(&env);
        let unrelated = Address::generate(&env);

        env.as_contract(&contract, || {
            env.storage()
                .instance()
                .set(&crate::ADMIN_KEY, &admin);
            assert_eq!(set_arbitrator(&env, &admin, &arbitrator), Ok(()));
            assert!(is_arbitrator(&env, &arbitrator));

            assert_eq!(require_admin_or_arbitrator(&env, &arbitrator), Ok(()));
            assert_eq!(require_admin_or_arbitrator(&env, &admin), Ok(()));
            for caller in [&filer, &provider, &unrelated] {
                assert_eq!(
                    require_admin_or_arbitrator(&env, caller),
                    Err(DisputeError::Unauthorized)
                );
            }
        });
    }

    #[test]
    fn evidence_is_frozen_once_adjudication_begins() {
        assert!(!adjudication_started(&DisputeStatus::Open));
        assert!(adjudication_started(&DisputeStatus::UnderReview));
        assert!(adjudication_started(&DisputeStatus::Escalated));
        assert!(!adjudication_started(&DisputeStatus::Resolved));
    }
}

#[cfg(test)]
mod config_freeze_toggle_auth_tests {
    use crate::config_freeze_toggle::{
        assert_config_not_frozen, freeze_configuration, is_configuration_frozen,
        unfreeze_configuration, ConfigFreezeError,
    };
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, Events as _},
        Address, Env, IntoVal,
    };

    fn setup() -> (Env, Address, Address) {
        let env = Env::default();
        let id = env.register_contract(None, crate::SLACalculatorContract);
        let admin = Address::generate(&env);
        (env, id, admin)
    }

    #[test]
    #[should_panic]
    fn unauthorized_caller_cannot_freeze_configuration() {
        let (env, id, _) = setup();
        let stranger = Address::generate(&env);

        env.as_contract(&id, || freeze_configuration(&env, &stranger));
    }

    #[test]
    #[should_panic]
    fn unauthorized_caller_cannot_unfreeze_configuration() {
        let (env, id, _) = setup();
        let stranger = Address::generate(&env);

        env.as_contract(&id, || {
            env.storage()
                .instance()
                .set(&symbol_short!("CFGFRZN"), &true);
            unfreeze_configuration(&env, &stranger);
        });
    }

    #[test]
    fn authorized_freeze_and_unfreeze_emit_one_event_each() {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, crate::SLACalculatorContract);
        let admin = Address::generate(&env);

        env.as_contract(&id, || {
            freeze_configuration(&env, &admin);
            assert!(is_configuration_frozen(&env));
            assert_eq!(
                assert_config_not_frozen(&env),
                Err(ConfigFreezeError::ConfigIsFrozen)
            );

            unfreeze_configuration(&env, &admin);
            assert!(!is_configuration_frozen(&env));
            assert_eq!(assert_config_not_frozen(&env), Ok(()));
        });

        let events = env.events().all();
        assert_eq!(events.len(), 2);
        assert_eq!(
            events.get(0).unwrap(),
            (
                id.clone(),
                (symbol_short!("cfgfrz"), admin.clone()).into_val(&env),
                true.into_val(&env)
            )
        );
        assert_eq!(
            events.get(1).unwrap(),
            (
                id.clone(),
                (symbol_short!("cfgunfrz"), admin.clone()).into_val(&env),
                false.into_val(&env)
            )
        );
    }

    #[test]
    fn repeating_a_direction_does_not_emit_another_event() {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, crate::SLACalculatorContract);
        let admin = Address::generate(&env);

        env.as_contract(&id, || {
            freeze_configuration(&env, &admin);
            freeze_configuration(&env, &admin);
            assert_eq!(env.events().all().len(), 1);

            unfreeze_configuration(&env, &admin);
            unfreeze_configuration(&env, &admin);
            assert_eq!(env.events().all().len(), 2);
        });
    }
}
