mod finite_automaton;
mod regular_expression;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

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

pub fn min_word_len_exactly_symbol_count(
    automaton: &FiniteAutomaton,
    symbol: char,
    count: usize,
) -> (bool, usize) {
    type SearchState = (AutomatonState, AutomatonTransition, usize, usize);

    let mut queue = VecDeque::<SearchState>::new();
    let mut curr_level = automaton.start_states.len();
    let mut next_level = 0;
    let mut depth = 0_usize;

    let mut is_found = false;
    let states_num = automaton.transitions.len();

    automaton
        .start_states
        .iter()
        .for_each(|state| queue.push_back((*state, AutomatonTransition::Epsilon, 0, 0)));

    while !queue.is_empty() {
        // SAFETY: queue is guaranteed not to be empty
        let (curr_state, curr_symbol, mut curr_count, mut curr_last_met) =
            queue.pop_front().unwrap();

        curr_level -= 1;
        curr_last_met += 1;

        if let AutomatonTransition::Symbol(curr_symbol) = curr_symbol {
            if curr_symbol == symbol {
                curr_count += 1;
                curr_last_met = 0;
            }
        };

        if curr_count > count || curr_last_met > states_num {
            continue;
        }

        if curr_count == count && automaton.accept_states.contains(&curr_state) {
            depth += (curr_level == 0) as usize;
            is_found = true;
            break;
        }

        // SAFETY: curr_state is guaranteed to be in nfa
        automaton
            .transitions
            .get(&curr_state)
            .unwrap()
            .iter()
            .for_each(|(sym, transition)| {
                transition.iter().for_each(|state| {
                    queue.push_back((*state, *sym, curr_count, curr_last_met));
                    next_level += 1;
                })
            });

        if curr_level == 0 {
            curr_level = next_level;
            next_level = 0;
            depth += 1;
        }
    }

    (is_found, depth - 1)
}
