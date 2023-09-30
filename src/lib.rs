mod finite_automaton;
mod regular_expression;

use std::collections::{BTreeMap, BTreeSet, HashSet};

pub type RegexEntry = Option<Box<RegexOps>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegexOps {
    Either(RegexEntry, RegexEntry),
    Consecutive(RegexEntry, RegexEntry),
    Repeat(RegexEntry),
    Symbol(char),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Regex {
    root: RegexEntry,
}

impl Default for Regex {
    fn default() -> Self {
        Self { root: None }
    }
}

pub type AutomatonState = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AutomatonTransition {
    Epsilon,
    Symbol(char),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AutomatonKind {
    Dfa,
    Nfa,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FiniteAutomaton {
    is_full: bool,
    start_state: usize,
    accept_states: HashSet<usize>,
    transitions: BTreeMap<AutomatonState, BTreeMap<AutomatonTransition, BTreeSet<AutomatonState>>>,
}
