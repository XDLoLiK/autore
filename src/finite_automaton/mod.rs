use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    ops::Deref,
};

use super::{AutomatonState, AutomatonTransition, FiniteAutomaton, Regex, RegexEntry, RegexOps};

impl FiniteAutomaton {
    pub fn from_regex(regex: &Regex) -> Self {
        match regex.root.as_ref() {
            Some(root) => {
                let mut nfa = Self::default();
                nfa.start_state = nfa.new_state();
                let accept_state = nfa.new_state();
                nfa.accept_states.insert(accept_state);
                nfa.traverse_regex(&root, nfa.start_state, accept_state);
                nfa
            }
            None => Self::default(),
        }
    }

    fn traverse_regex(
        &mut self,
        curr_op: &RegexEntry,
        start_state: AutomatonState,
        accept_state: AutomatonState,
    ) {
        match curr_op.deref() {
            RegexOps::Either(left, right) => {
                let left_start = self.new_state();
                let left_accept = self.new_state();
                self.add_transition(start_state, AutomatonTransition::Epsilon, left_start);
                self.add_transition(left_accept, AutomatonTransition::Epsilon, accept_state);
                self.traverse_regex(left, left_start, left_accept);

                let right_start = self.new_state();
                let right_accept = self.new_state();
                self.add_transition(start_state, AutomatonTransition::Epsilon, right_start);
                self.add_transition(right_accept, AutomatonTransition::Epsilon, accept_state);
                self.traverse_regex(right, right_start, right_accept);
            }
            RegexOps::Consecutive(left, right) => {
                let inbetween = self.new_state();
                self.traverse_regex(left, start_state, inbetween);
                self.traverse_regex(right, inbetween, accept_state);
            }
            RegexOps::Repeat(what) => {
                let repeat_start = self.new_state();
                let repeat_accept = self.new_state();
                self.add_transition(start_state, AutomatonTransition::Epsilon, repeat_start);
                self.add_transition(start_state, AutomatonTransition::Epsilon, accept_state);
                self.add_transition(repeat_accept, AutomatonTransition::Epsilon, accept_state);
                self.add_transition(repeat_accept, AutomatonTransition::Epsilon, repeat_start);
                self.traverse_regex(what, repeat_start, repeat_accept);
            }
            RegexOps::Symbol(sym) => {
                self.add_transition(start_state, AutomatonTransition::Symbol(*sym), accept_state);
            }
            RegexOps::Epsilon => {
                self.add_transition(start_state, AutomatonTransition::Epsilon, accept_state);
            }
        }
    }

    fn add_transition(
        &mut self,
        src: AutomatonState,
        trans: AutomatonTransition,
        dest: AutomatonState,
    ) {
        let trans_list = self.transitions.entry(src).or_default();
        let dest_list = trans_list.entry(trans).or_default();
        dest_list.insert(dest);
    }

    pub fn eliminate_epsilon(&mut self) {
        
    }

    pub fn to_dfa(nfa: &FiniteAutomaton) -> Self {
        if nfa.transitions.is_empty() {
            return Self::default();
        }

        let mut dfa = Self::default();
        let mut queue = VecDeque::<AutomatonState>::new();
        let mut used = HashSet::<AutomatonState>::new();
        let mut mapping = HashMap::<AutomatonState, BTreeSet<AutomatonState>>::new();
        let mut reverse_mapping = HashMap::<BTreeSet<AutomatonState>, AutomatonState>::new();

        dfa.start_state = dfa.new_state();
        queue.push_back(dfa.start_state);
        mapping.insert(dfa.start_state, BTreeSet::from([nfa.start_state]));
        reverse_mapping.insert(BTreeSet::from([nfa.start_state]), dfa.start_state);

        while !queue.is_empty() {
            // It is safe unwrap here as queue is guaranteed not to be empty
            let curr_state = queue.pop_front().unwrap();

            if used.contains(&curr_state) {
                continue;
            }

            // It is safe to unwrap here because every queued state is mapped to some nfa states
            let curr_mapped_to = mapping.get(&curr_state).unwrap();
            let mut dfa_nfa_trans =
                BTreeMap::<AutomatonTransition, BTreeSet<AutomatonState>>::new();

            // Collect info about (dfa_state - char - nfa_states) transitions
            // in order to later convert it into (dfa_state - char - dfa_state) transitions
            for nfa_state in curr_mapped_to.iter() {
                if nfa.accept_states.contains(nfa_state) {
                    dfa.accept_states.insert(curr_state);
                }

                // It is safe to unwrap here because nfa_state is guaranteed to be in nfa
                let nfa_trans = nfa.transitions.get(nfa_state).unwrap();

                for (symbol, nfa_to) in nfa_trans.iter() {
                    let entry = dfa_nfa_trans.entry(*symbol).or_default();
                    entry.extend(nfa_to);
                }
            }

            for (symbol, nfa_to) in dfa_nfa_trans.iter() {
                let dfa_to = match reverse_mapping.get(&nfa_to) {
                    Some(mapped_dfa) => *mapped_dfa,
                    None => {
                        let new_dfa = dfa.new_state();
                        mapping.insert(new_dfa, nfa_to.clone());
                        reverse_mapping.insert(nfa_to.clone(), new_dfa);
                        queue.push_back(new_dfa);
                        new_dfa
                    }
                };

                dfa.add_transition(curr_state, *symbol, dfa_to);
            }

            used.insert(curr_state);
        }

        dfa
    }

    fn new_state(&mut self) -> usize {
        let new_state = self.transitions.len();
        self.transitions.insert(new_state, BTreeMap::default());
        new_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfa_to_dfa_unit_1() {
        let nfa = FiniteAutomaton {
            start_state: 0,
            accept_states: HashSet::from([2]),
            transitions: BTreeMap::from([
                (
                    0,
                    BTreeMap::from([
                        (AutomatonTransition::Symbol('a'), BTreeSet::from([0])),
                        (AutomatonTransition::Symbol('b'), BTreeSet::from([1])),
                    ]),
                ),
                (
                    1,
                    BTreeMap::from([
                        (AutomatonTransition::Symbol('a'), BTreeSet::from([1, 2])),
                        (AutomatonTransition::Symbol('b'), BTreeSet::from([1])),
                    ]),
                ),
                (
                    2,
                    BTreeMap::from([
                        (AutomatonTransition::Symbol('a'), BTreeSet::from([2])),
                        (AutomatonTransition::Symbol('b'), BTreeSet::from([1, 2])),
                    ]),
                ),
            ]),
        };

        let dfa = FiniteAutomaton::to_dfa(&nfa);

        assert_eq!(
            dfa,
            FiniteAutomaton {
                start_state: 0,
                accept_states: HashSet::from([2]),
                transitions: BTreeMap::from([
                    (
                        0,
                        BTreeMap::from([
                            (AutomatonTransition::Symbol('a'), BTreeSet::from([0])),
                            (AutomatonTransition::Symbol('b'), BTreeSet::from([1]))
                        ]),
                    ),
                    (
                        1,
                        BTreeMap::from([
                            (AutomatonTransition::Symbol('a'), BTreeSet::from([2])),
                            (AutomatonTransition::Symbol('b'), BTreeSet::from([1]))
                        ]),
                    ),
                    (
                        2,
                        BTreeMap::from([
                            (AutomatonTransition::Symbol('a'), BTreeSet::from([2])),
                            (AutomatonTransition::Symbol('b'), BTreeSet::from([2]))
                        ]),
                    ),
                ]),
            }
        );
    }

    #[test]
    fn nfa_to_dfa_unit_2() {
        let nfa = FiniteAutomaton {
            start_state: 0,
            accept_states: HashSet::from([2]),
            transitions: BTreeMap::from([
                (
                    0,
                    BTreeMap::from([
                        (AutomatonTransition::Symbol('a'), BTreeSet::from([0])),
                        (AutomatonTransition::Symbol('b'), BTreeSet::from([0, 1])),
                    ]),
                ),
                (
                    1,
                    BTreeMap::from([(AutomatonTransition::Symbol('a'), BTreeSet::from([2]))]),
                ),
                (
                    2,
                    BTreeMap::from([(AutomatonTransition::Symbol('a'), BTreeSet::from([]))]),
                ),
            ]),
        };

        let dfa = FiniteAutomaton::to_dfa(&nfa);

        assert_eq!(
            dfa,
            FiniteAutomaton {
                start_state: 0,
                accept_states: HashSet::from([2]),
                transitions: BTreeMap::from([
                    (
                        0,
                        BTreeMap::from([
                            (AutomatonTransition::Symbol('a'), BTreeSet::from([0])),
                            (AutomatonTransition::Symbol('b'), BTreeSet::from([1]))
                        ]),
                    ),
                    (
                        1,
                        BTreeMap::from([
                            (AutomatonTransition::Symbol('a'), BTreeSet::from([2])),
                            (AutomatonTransition::Symbol('b'), BTreeSet::from([1]))
                        ]),
                    ),
                    (
                        2,
                        BTreeMap::from([
                            (AutomatonTransition::Symbol('a'), BTreeSet::from([0])),
                            (AutomatonTransition::Symbol('b'), BTreeSet::from([1]))
                        ]),
                    ),
                ]),
            }
        );
    }

    #[test]
    fn nfa_to_dfa_unit_3() {
        let nfa = FiniteAutomaton {
            start_state: 0,
            accept_states: HashSet::from([5]),
            transitions: BTreeMap::from([
                (
                    0,
                    BTreeMap::from([
                        (AutomatonTransition::Symbol('a'), BTreeSet::from([1])),
                        (AutomatonTransition::Symbol('b'), BTreeSet::from([1])),
                    ]),
                ),
                (
                    1,
                    BTreeMap::from([
                        (AutomatonTransition::Symbol('a'), BTreeSet::from([1])),
                        (AutomatonTransition::Symbol('b'), BTreeSet::from([1, 2])),
                    ]),
                ),
                (
                    2,
                    BTreeMap::from([(AutomatonTransition::Symbol('a'), BTreeSet::from([3]))]),
                ),
                (
                    3,
                    BTreeMap::from([(AutomatonTransition::Symbol('b'), BTreeSet::from([4]))]),
                ),
                (
                    4,
                    BTreeMap::from([
                        (AutomatonTransition::Symbol('a'), BTreeSet::from([5])),
                        (AutomatonTransition::Symbol('b'), BTreeSet::from([5])),
                    ]),
                ),
                (
                    5,
                    BTreeMap::from([
                        (AutomatonTransition::Symbol('a'), BTreeSet::from([5])),
                        (AutomatonTransition::Symbol('b'), BTreeSet::from([5])),
                    ]),
                ),
            ]),
        };

        let dfa = FiniteAutomaton::to_dfa(&nfa);

        assert_eq!(
            dfa,
            FiniteAutomaton {
                start_state: 0,
                accept_states: HashSet::from([5, 6, 7, 8]),
                transitions: BTreeMap::from([
                    (
                        0,
                        BTreeMap::from([
                            (AutomatonTransition::Symbol('a'), BTreeSet::from([1])),
                            (AutomatonTransition::Symbol('b'), BTreeSet::from([1]))
                        ]),
                    ),
                    (
                        1,
                        BTreeMap::from([
                            (AutomatonTransition::Symbol('a'), BTreeSet::from([1])),
                            (AutomatonTransition::Symbol('b'), BTreeSet::from([2]))
                        ]),
                    ),
                    (
                        2,
                        BTreeMap::from([
                            (AutomatonTransition::Symbol('a'), BTreeSet::from([3])),
                            (AutomatonTransition::Symbol('b'), BTreeSet::from([2]))
                        ]),
                    ),
                    (
                        3,
                        BTreeMap::from([
                            (AutomatonTransition::Symbol('a'), BTreeSet::from([1])),
                            (AutomatonTransition::Symbol('b'), BTreeSet::from([4]))
                        ]),
                    ),
                    (
                        4,
                        BTreeMap::from([
                            (AutomatonTransition::Symbol('a'), BTreeSet::from([5])),
                            (AutomatonTransition::Symbol('b'), BTreeSet::from([6]))
                        ]),
                    ),
                    (
                        5,
                        BTreeMap::from([
                            (AutomatonTransition::Symbol('a'), BTreeSet::from([7])),
                            (AutomatonTransition::Symbol('b'), BTreeSet::from([8]))
                        ]),
                    ),
                    (
                        6,
                        BTreeMap::from([
                            (AutomatonTransition::Symbol('a'), BTreeSet::from([5])),
                            (AutomatonTransition::Symbol('b'), BTreeSet::from([6]))
                        ]),
                    ),
                    (
                        7,
                        BTreeMap::from([
                            (AutomatonTransition::Symbol('a'), BTreeSet::from([7])),
                            (AutomatonTransition::Symbol('b'), BTreeSet::from([6]))
                        ]),
                    ),
                    (
                        8,
                        BTreeMap::from([
                            (AutomatonTransition::Symbol('a'), BTreeSet::from([5])),
                            (AutomatonTransition::Symbol('b'), BTreeSet::from([6]))
                        ]),
                    ),
                ]),
            }
        );
    }
}
