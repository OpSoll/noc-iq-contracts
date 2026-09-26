use soroban_sdk::{contracttype, symbol_short, Env, Map, Symbol, Vec};

pub const CORRELATION_INDEX: Symbol = symbol_short!("corr_idx");
pub const CHILD_INDEX: Symbol = symbol_short!("chd_idx");

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct OutageTag {
    pub outage_id: Symbol,
    pub parent_outage_id: Option<Symbol>,
    pub downtime_seconds: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CorrelationError {
    SelfParent,
    CycleDetected,
}

pub fn validate_parent(
    outage_id: &Symbol,
    parent_outage_id: &Option<Symbol>,
) -> Result<(), CorrelationError> {
    if let Some(parent) = parent_outage_id {
        if parent == outage_id {
            return Err(CorrelationError::SelfParent);
        }
    }
    Ok(())
}

pub const MAX_CORRELATION_DEPTH: u32 = 64;

pub fn immediate_parent(tag: &OutageTag) -> Option<Symbol> {
    tag.parent_outage_id.clone()
}

pub fn resolve_root(env: &Env, outage_id: &Symbol) -> Symbol {
    let tags = correlation_index(env);
    let mut current = outage_id.clone();
    let mut depth = 0u32;
    while depth < MAX_CORRELATION_DEPTH {
        match tags.get(current.clone()) {
            Some(tag) => match tag.parent_outage_id {
                Some(parent) => current = parent,
                None => return current,
            },
            None => return current,
        }
        depth += 1;
    }
    current
}

fn correlation_index(env: &Env) -> Map<Symbol, OutageTag> {
    env.storage()
        .persistent()
        .get(&CORRELATION_INDEX)
        .unwrap_or_else(|| Map::new(env))
}

fn child_index(env: &Env) -> Map<Symbol, Vec<Symbol>> {
    env.storage()
        .persistent()
        .get(&CHILD_INDEX)
        .unwrap_or_else(|| Map::new(env))
}

fn ancestry_contains(env: &Env, start: &Symbol, target: &Symbol) -> bool {
    let tags = correlation_index(env);
    let mut cursor = Some(start.clone());
    while let Some(current) = cursor {
        if current == *target {
            return true;
        }
        cursor = tags.get(current).and_then(|tag| tag.parent_outage_id);
    }
    false
}

pub fn would_create_cycle(env: &Env, outage_id: &Symbol, parent: &Symbol) -> bool {
    ancestry_contains(env, parent, outage_id)
}

pub fn tag_outage(env: &Env, tag: OutageTag) -> Result<Symbol, CorrelationError> {
    validate_parent(&tag.outage_id, &tag.parent_outage_id)?;
    if let Some(parent) = &tag.parent_outage_id {
        if would_create_cycle(env, &tag.outage_id, parent) {
            return Err(CorrelationError::CycleDetected);
        }
    }

    let outage_id = tag.outage_id.clone();
    let parent_outage_id = tag.parent_outage_id.clone();
    let mut tags = correlation_index(env);
    tags.set(outage_id.clone(), tag.clone());
    env.storage().persistent().set(&CORRELATION_INDEX, &tags);

    if let Some(parent) = parent_outage_id {
        let mut children = child_index(env);
        let mut siblings = children
            .get(parent.clone())
            .unwrap_or_else(|| Vec::new(env));
        if !siblings.contains(&outage_id) {
            siblings.push_back(outage_id.clone());
        }
        children.set(parent, siblings);
        env.storage().persistent().set(&CHILD_INDEX, &children);
    }

    Ok(resolve_root(env, &outage_id))
}

pub fn load_tag(env: &Env, outage_id: &Symbol) -> Option<OutageTag> {
    correlation_index(env).get(outage_id.clone())
}

pub fn children_of(env: &Env, parent: &Symbol) -> Vec<Symbol> {
    child_index(env)
        .get(parent.clone())
        .unwrap_or_else(|| Vec::new(env))
}

pub fn group_by_root(env: &Env) -> Map<Symbol, Vec<Symbol>> {
    let mut groups: Map<Symbol, Vec<Symbol>> = Map::new(env);
    for (outage_id, _tag) in correlation_index(env).iter() {
        let root = resolve_root(env, &outage_id);
        let mut members = groups.get(root.clone()).unwrap_or_else(|| Vec::new(env));
        if !members.contains(&outage_id) {
            members.push_back(outage_id);
        }
        groups.set(root, members);
    }
    groups
}

pub fn aggregate_root_downtime(env: &Env, root: &Symbol) -> u64 {
    let members = group_by_root(env)
        .get(root.clone())
        .unwrap_or_else(|| Vec::new(env));
    let mut total: u64 = 0;
    for member in members.iter() {
        if let Some(tag) = load_tag(env, &member) {
            total = total.saturating_add(tag.downtime_seconds);
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;

    fn contract(env: &Env) -> soroban_sdk::Address {
        env.register_contract(None, SLACalculatorContract)
    }

    fn id(env: &Env, text: &str) -> Symbol {
        Symbol::new(env, text)
    }

    fn tag(env: &Env, outage: &str, parent: Option<&str>, downtime: u64) -> OutageTag {
        OutageTag {
            outage_id: id(env, outage),
            parent_outage_id: parent.map(|text| id(env, text)),
            downtime_seconds: downtime,
        }
    }

    #[test]
    fn a_root_outage_has_no_children() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let root = id(&env, "OUT1");
            tag_outage(&env, tag(&env, "OUT1", None, 600)).expect("root outage");
            assert_eq!(children_of(&env, &root).len(), 0);
        });
    }

    #[test]
    fn a_self_parent_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            tag_outage(&env, tag(&env, "OUT1", Some("OUT1"), 10))
        });
        assert_eq!(result, Err(CorrelationError::SelfParent));
    }

    #[test]
    fn a_child_resolves_to_its_parent_root() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let root = id(&env, "OUT1");
            let child = id(&env, "OUT2");
            let resolved =
                tag_outage(&env, tag(&env, "OUT2", Some("OUT1"), 120)).expect("child outage");
            assert_eq!(resolved, root);
            let stored = load_tag(&env, &child).expect("stored tag");
            assert_eq!(stored.parent_outage_id, Some(root.clone()));
            assert_eq!(children_of(&env, &root).len(), 1);
        });
    }

    #[test]
    fn cascading_failures_group_under_one_root() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let root = id(&env, "OUT1");
            tag_outage(&env, tag(&env, "OUT1", None, 600)).expect("root");
            tag_outage(&env, tag(&env, "OUT2", Some("OUT1"), 300)).expect("child a");
            tag_outage(&env, tag(&env, "OUT3", Some("OUT1"), 100)).expect("child b");
            let groups = group_by_root(&env);
            assert_eq!(groups.len(), 1);
            assert_eq!(groups.get(root).expect("root group").len(), 3);
        });
    }

    #[test]
    fn child_downtime_folds_into_the_root_duration() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let root = id(&env, "OUT1");
            tag_outage(&env, tag(&env, "OUT1", None, 600)).expect("root");
            tag_outage(&env, tag(&env, "OUT2", Some("OUT1"), 250)).expect("child");
            assert_eq!(aggregate_root_downtime(&env, &root), 850);
        });
    }

    #[test]
    fn a_nested_chain_resolves_to_the_original_root() {
        let env = Env::default();
        let expected = id(&env, "OUT1");
        let resolved = env.as_contract(&contract(&env), || {
            tag_outage(&env, tag(&env, "OUT2", Some("OUT1"), 10)).expect("child");
            tag_outage(&env, tag(&env, "OUT3", Some("OUT2"), 5)).expect("grandchild")
        });
        assert_eq!(resolved, expected);
    }

    #[test]
    fn a_cycle_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            tag_outage(&env, tag(&env, "OUT2", Some("OUT1"), 10)).expect("child");
            tag_outage(&env, tag(&env, "OUT1", Some("OUT2"), 10))
        });
        assert_eq!(result, Err(CorrelationError::CycleDetected));
    }

    #[test]
    fn re_tagging_the_same_outage_does_not_duplicate_children() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let root = id(&env, "OUT1");
            for _ in 0..2 {
                tag_outage(&env, tag(&env, "OUT2", Some("OUT1"), 60)).expect("child");
            }
            assert_eq!(children_of(&env, &root).len(), 1);
        });
    }

    #[test]
    fn an_unknown_outage_has_no_tag() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            assert!(load_tag(&env, &id(&env, "OUT404")).is_none());
        });
    }
}
