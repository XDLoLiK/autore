use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

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

    pub fn eliminate_epsilon(&mut self) {}
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
    /// nfa.eliminate_epsilon();
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
            let curr_mapped_to = mapping.get(&curr_idx).unwrap();
            let mut new_trans = BTreeMap::<char, BTreeSet<usize>>::new();

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

            for (symbol, nfa_to) in new_trans.iter() {
                // It is safe to unwrap here as we already know that there is such a symbol in keys
                let nfa_to: Vec<_> = nfa_to.clone().into_iter().collect();

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

    #[test]
    fn nfa_to_dfa_unit_3() {
        let nfa = Nfa {
            start_state: 0,
            states: vec![
                NfaState {
                    is_final: false,
                    transitions: BTreeMap::from([('a', vec![1]), ('b', vec![1])]),
                },
                NfaState {
                    is_final: false,
                    transitions: BTreeMap::from([('a', vec![1]), ('b', vec![1, 2])]),
                },
                NfaState {
                    is_final: false,
                    transitions: BTreeMap::from([('a', vec![3])]),
                },
                NfaState {
                    is_final: false,
                    transitions: BTreeMap::from([('a', vec![]), ('b', vec![4])]),
                },
                NfaState {
                    is_final: false,
                    transitions: BTreeMap::from([('a', vec![5]), ('b', vec![5])]),
                },
                NfaState {
                    is_final: true,
                    transitions: BTreeMap::from([('a', vec![5]), ('b', vec![5])]),
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
                        transitions: BTreeMap::from([('a', 1), ('b', 1)]),
                    },
                    DfaState {
                        is_final: false,
                        transitions: BTreeMap::from([('a', 1), ('b', 2)]),
                    },
                    DfaState {
                        is_final: false,
                        transitions: BTreeMap::from([('a', 3), ('b', 2)]),
                    },
                    DfaState {
                        is_final: false,
                        transitions: BTreeMap::from([('a', 1), ('b', 4)]),
                    },
                    DfaState {
                        is_final: false,
                        transitions: BTreeMap::from([('a', 5), ('b', 6)]),
                    },
                    DfaState {
                        is_final: true,
                        transitions: BTreeMap::from([('a', 7), ('b', 8)]),
                    },
                    DfaState {
                        is_final: true,
                        transitions: BTreeMap::from([('a', 5), ('b', 6)]),
                    },
                    DfaState {
                        is_final: true,
                        transitions: BTreeMap::from([('a', 7), ('b', 6)]),
                    },
                    DfaState {
                        is_final: true,
                        transitions: BTreeMap::from([('a', 5), ('b', 6)]),
                    },
                ],
            }
        );
    }
}
