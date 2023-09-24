use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

pub const EPSILON_TRANSITION: char = '\u{03B5}';

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NfaState {
    is_final: bool,
    transitions: BTreeMap<char, Vec<usize>>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DfaState {
    is_final: bool,
    transitions: BTreeMap<char, usize>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Nfa {
    start_state: usize,
    states: Vec<NfaState>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Dfa {
    start_state: usize,
    states: Vec<DfaState>,
}

impl Nfa {
    pub fn from_regex() -> Self {
        Self {
            start_state: 0,
            states: vec![],
        }
    }

    pub fn simplify(&mut self) {}
}

impl Dfa {
    /// Converts nfa to dfa.
    ///
    /// # Note
    ///
    /// This function implies that there are no epsilon transitions in the given nfa.\
    /// Otherwise the output dfa may be incorrect.
    ///
    /// # Examples
    ///
    /// ```
    /// use autore::finite_automaton::{Nfa, Dfa};
    ///
    /// let mut nfa = Nfa::default();
    /// nfa.simplify();
    ///
    /// let dfa = Dfa::from_nfa(&nfa);
    /// ```
    pub fn from_nfa(nfa: &Nfa) -> Self {
        if nfa.states.is_empty() {
            return Self::default();
        }

        let mut dfa = Self::default();
        let mut queue = VecDeque::<usize>::new();
        let mut used = HashSet::<usize>::new();
        let mut mapping = HashMap::<usize, Vec<usize>>::new();
        let mut reverse_mapping = HashMap::<Vec<usize>, usize>::new();

        dfa.start_state = dfa.new_state();
        queue.push_back(dfa.start_state);
        mapping.insert(0, vec![nfa.start_state]);
        reverse_mapping.insert(vec![nfa.start_state], 0);

        while !queue.is_empty() {
            // It is safe unwrap here as queue is guaranteed not to be empty
            let curr_idx = queue.pop_front().unwrap();

            if used.contains(&curr_idx) {
                continue;
            }

            // It is safe to unwrap here because every queued state is mapped to some nfa state(s)
            let curr_mapped_to = mapping.get(&curr_idx).unwrap().clone();
            let mut new_trans = HashMap::<char, BTreeSet<usize>>::new();

            for nfa_idx in curr_mapped_to.iter() {
                let nfa_state = &nfa.states[*nfa_idx];
                dfa.states[curr_idx].is_final |= nfa_state.is_final;

                for (symbol, nfa_to) in nfa_state.transitions.iter() {
                    match new_trans.get_mut(symbol) {
                        Some(existing_set) => {
                            existing_set.extend(nfa_to.iter());
                        }
                        None => {
                            let mut new_set = BTreeSet::<usize>::new();
                            new_set.extend(nfa_to.iter());
                            new_trans.insert(*symbol, new_set);
                        }
                    }
                }
            }

            for symbol in new_trans.keys() {
                // It is safe to unwrap here as we already know that there is
                // such a symbol in keys
                let nfa_to: Vec<_> = new_trans.get(symbol).unwrap().clone().into_iter().collect();

                let dfa_to = match reverse_mapping.get(&nfa_to) {
                    Some(mapped_dfa) => *mapped_dfa,
                    None => {
                        let new_state = dfa.new_state();
                        mapping.insert(new_state, nfa_to.clone());
                        reverse_mapping.insert(nfa_to.clone(), new_state);
                        queue.push_back(new_state);
                        new_state
                    }
                };

                dfa.states[curr_idx].transitions.insert(*symbol, dfa_to);
            }

            used.insert(curr_idx);
        }

        dfa
    }

    fn new_state(&mut self) -> usize {
        self.states.push(DfaState::default());
        self.states.len() - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfa_to_dfa_unit_1() {
        let nfa = Nfa {
            start_state: 0,
            states: vec![
                NfaState {
                    is_final: false,
                    transitions: BTreeMap::from([('0', vec![0]), ('1', vec![1])]),
                },
                NfaState {
                    is_final: false,
                    transitions: BTreeMap::from([('0', vec![1, 2]), ('1', vec![1])]),
                },
                NfaState {
                    is_final: true,
                    transitions: BTreeMap::from([('0', vec![2]), ('1', vec![1, 2])]),
                },
            ],
        };

        let dfa = Dfa::from_nfa(&nfa);

        assert_eq!(
            dfa,
            Dfa {
                start_state: 0,
                states: vec![
                    DfaState {
                        is_final: false,
                        transitions: BTreeMap::from([('0', 0), ('1', 1)]),
                    },
                    DfaState {
                        is_final: false,
                        transitions: BTreeMap::from([('0', 2), ('1', 1)]),
                    },
                    DfaState {
                        is_final: true,
                        transitions: BTreeMap::from([('0', 2), ('1', 2)]),
                    },
                ],
            }
        )
    }

    #[test]
    fn nfa_to_dfa_unit_2() {
        let nfa = Nfa {
            start_state: 0,
            states: vec![
                NfaState {
                    is_final: false,
                    transitions: BTreeMap::from([('0', vec![0]), ('1', vec![0, 1])]),
                },
                NfaState {
                    is_final: false,
                    transitions: BTreeMap::from([('0', vec![2]), ('1', vec![])]),
                },
                NfaState {
                    is_final: true,
                    transitions: BTreeMap::from([('0', vec![]), ('1', vec![])]),
                },
            ],
        };

        let dfa = Dfa::from_nfa(&nfa);

        assert_eq!(
            dfa,
            Dfa {
                start_state: 0,
                states: vec![
                    DfaState {
                        is_final: false,
                        transitions: BTreeMap::from([('0', 0), ('1', 1)]),
                    },
                    DfaState {
                        is_final: false,
                        transitions: BTreeMap::from([('0', 2), ('1', 1)]),
                    },
                    DfaState {
                        is_final: true,
                        transitions: BTreeMap::from([('0', 0), ('1', 1)]),
                    },
                ],
            }
        )
    }
}
