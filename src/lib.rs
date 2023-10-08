mod finite_automaton;
mod regular_expression;

use std::collections::{BTreeMap, BTreeSet};

pub type RegexEntry = Box<RegexOps>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RegexOps {
    Either(RegexEntry, RegexEntry),
    Consecutive(RegexEntry, RegexEntry),
    NoneOrMore(RegexEntry),
    NoneOrOnce(RegexEntry),
    OnceOrMore(RegexEntry),
    Symbol(char),
    Epsilon,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Regex {
    root: Option<RegexEntry>,
}

impl Default for Regex {
    fn default() -> Self {
        Self { root: None }
    }
}

pub type AutomatonState = usize;
pub type AutomatonTransitionList = BTreeMap<AutomatonTransition, BTreeSet<AutomatonState>>;
pub type AutomatonAlphabet = BTreeSet<AutomatonTransition>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AutomatonTransition {
    Epsilon,
    Symbol(char),
}

// Use BTree here instead of Hash to get determenistic results every time
#[derive(Debug, Default, Clone)]
pub struct FiniteAutomaton {
    last_state: AutomatonState,
    start_states: BTreeSet<AutomatonState>,
    accept_states: BTreeSet<AutomatonState>,
    transitions: BTreeMap<AutomatonState, AutomatonTransitionList>,
}
