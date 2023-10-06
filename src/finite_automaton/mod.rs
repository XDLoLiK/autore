use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    fs::File,
    io::{self, BufWriter, Write},
    ops::Deref,
    process::Command,
};

use tabbycat::attributes::*;
use tabbycat::{AttrList, AttrType, Edge, GraphBuilder, GraphType, Identity, StmtList};

use super::{
    AutomatonAlphabet, AutomatonState, AutomatonTransition, AutomatonTransitionList,
    FiniteAutomaton, Regex, RegexEntry, RegexOps,
};

impl FiniteAutomaton {
    pub fn from_regex(regex: &Regex) -> Self {
        match regex.root.as_ref() {
            Some(root) => {
                let mut nfa = Self::default();
                let start_state = nfa.add_state();
                nfa.start_states = BTreeSet::from([start_state]);
                let accept_state = nfa.add_state();
                nfa.accept_states.insert(accept_state);
                nfa.traverse_regex(&root, start_state, accept_state);
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
                let left_start = self.add_state();
                let left_accept = self.add_state();
                self.add_transition(start_state, AutomatonTransition::Epsilon, left_start);
                self.add_transition(left_accept, AutomatonTransition::Epsilon, accept_state);
                self.traverse_regex(left, left_start, left_accept);

                let right_start = self.add_state();
                let right_accept = self.add_state();
                self.add_transition(start_state, AutomatonTransition::Epsilon, right_start);
                self.add_transition(right_accept, AutomatonTransition::Epsilon, accept_state);
                self.traverse_regex(right, right_start, right_accept);
            }
            RegexOps::Consecutive(left, right) => {
                let inbetween = self.add_state();
                self.traverse_regex(left, start_state, inbetween);
                self.traverse_regex(right, inbetween, accept_state);
            }
            RegexOps::NoneOrMore(what) => {
                let repeat_start = self.add_state();
                let repeat_accept = self.add_state();
                self.add_transition(start_state, AutomatonTransition::Epsilon, repeat_start);
                self.add_transition(start_state, AutomatonTransition::Epsilon, accept_state);
                self.add_transition(repeat_accept, AutomatonTransition::Epsilon, accept_state);
                self.add_transition(repeat_accept, AutomatonTransition::Epsilon, repeat_start);
                self.traverse_regex(what, repeat_start, repeat_accept);
            }
            RegexOps::NoneOrOnce(what) => {
                let repeat_start = self.add_state();
                let repeat_accept = self.add_state();
                self.add_transition(start_state, AutomatonTransition::Epsilon, repeat_start);
                self.add_transition(start_state, AutomatonTransition::Epsilon, accept_state);
                self.add_transition(repeat_accept, AutomatonTransition::Epsilon, accept_state);
                self.traverse_regex(what, repeat_start, repeat_accept);
            }
            RegexOps::OnceOrMore(what) => {
                let repeat_start = self.add_state();
                let repeat_accept = self.add_state();
                self.add_transition(start_state, AutomatonTransition::Epsilon, repeat_start);
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

    pub fn eliminate_epsilon(&mut self) {
        // Step 1: Create an epsilon closure
        let mut closure_matrix = BTreeMap::<AutomatonState, BTreeMap<AutomatonState, bool>>::new();

        self.transitions
            .iter()
            .for_each(|(state, state_transitions)| {
                state_transitions
                    .get(&AutomatonTransition::Epsilon)
                    .unwrap_or(&BTreeSet::<AutomatonState>::new())
                    .iter()
                    .for_each(|epsilon_transition| {
                        closure_matrix
                            .entry(*state)
                            .or_default()
                            .insert(*epsilon_transition, true);
                    });
            });

        self.floyd_warshall(&mut closure_matrix);

        closure_matrix
            .iter()
            .for_each(|(from, epsilon_transitions)| {
                epsilon_transitions.iter().for_each(|(to, reachable)| {
                    if *reachable {
                        self.add_transition(*from, AutomatonTransition::Epsilon, *to);
                    };
                });
            });

        // Step 2: Update accept states
        self.transitions
            .iter()
            .for_each(|(state, state_transitions)| {
                state_transitions
                    .get(&AutomatonTransition::Epsilon)
                    .unwrap_or(&BTreeSet::<AutomatonState>::new())
                    .iter()
                    .for_each(|epsilon_state| {
                        if self.accept_states.contains(epsilon_state) {
                            self.accept_states.insert(*state);
                        }
                    });
            });

        // Step 3: Add (u - Epsilon - v - Symbol - w) edges
        let cloned_transitions = self.transitions.clone();

        cloned_transitions
            .iter()
            .for_each(|(state, state_transitions)| {
                state_transitions
                    .get(&AutomatonTransition::Epsilon)
                    .unwrap_or(&BTreeSet::<AutomatonState>::new())
                    .iter()
                    .for_each(|epsilon_transition| {
                        // SAFETY: every state must have been created via
                        // new_state() and thus is present in transitions map
                        cloned_transitions
                            .get(epsilon_transition)
                            .unwrap()
                            .iter()
                            .for_each(|(symbol, dest_states)| {
                                dest_states.iter().for_each(|dest_state| {
                                    self.add_transition(*state, *symbol, *dest_state);
                                });
                            });
                    });
            });

        // Step 4: Remove all the epsilon transitions from the automaton
        self.transitions.values_mut().for_each(|state_transitions| {
            state_transitions.remove(&AutomatonTransition::Epsilon);
        });

        // Step 5: Remove the dead states that could have appeared
        self.eliminate_dead();
    }

    fn floyd_warshall(
        &self,
        matrix: &mut BTreeMap<AutomatonState, BTreeMap<AutomatonState, bool>>,
    ) {
        self.transitions.keys().for_each(|k| {
            self.transitions.keys().for_each(|i| {
                self.transitions.keys().for_each(|j| {
                    let matrix_i_k = *matrix.entry(*i).or_default().entry(*k).or_default();
                    let matrix_k_j = *matrix.entry(*k).or_default().entry(*j).or_default();
                    let entry_i_j = matrix.entry(*i).or_default().entry(*j).or_default();
                    *entry_i_j = *entry_i_j | (matrix_i_k & matrix_k_j);
                });
            });
        });
    }

    pub fn eliminate_dead(&mut self) {
        let mut ref_count = BTreeMap::<AutomatonState, usize>::new();

        self.start_states.iter().for_each(|start_state| {
            *ref_count.entry(*start_state).or_default() += 1;
        });

        self.transitions.values().for_each(|state_transitions| {
            state_transitions.values().for_each(|dest_states| {
                dest_states.iter().for_each(|dest_state| {
                    *ref_count.entry(*dest_state).or_default() += 1;
                });
            });
        });

        self.transitions
            .clone()
            .keys()
            .filter(|state| ref_count.get(state).copied().unwrap_or_default() == 0)
            .for_each(|state| {
                self.remove_state(*state);
            });
    }

    pub fn to_dfa(nfa: &FiniteAutomaton) -> Self {
        let mut dfa = Self::default();
        let mut queue = VecDeque::<AutomatonState>::new();
        let mut used = HashSet::<AutomatonState>::new();
        let mut mapping = HashMap::<AutomatonState, BTreeSet<AutomatonState>>::new();
        let mut reverse_mapping = HashMap::<BTreeSet<AutomatonState>, AutomatonState>::new();

        let start_state = dfa.add_state();
        dfa.start_states = BTreeSet::from([start_state]);
        queue.push_back(start_state);
        mapping.insert(start_state, nfa.start_states.clone());
        reverse_mapping.insert(nfa.start_states.clone(), start_state);

        while !queue.is_empty() {
            // SAFETY: queue is guaranteed not to be empty
            let curr_state = queue.pop_front().unwrap();

            if used.contains(&curr_state) {
                continue;
            }

            // SAFETY: every queued state is mapped to some nfa states
            let curr_mapped_to = mapping.get(&curr_state).unwrap();
            let mut dfa_nfa_trans =
                BTreeMap::<AutomatonTransition, BTreeSet<AutomatonState>>::new();

            // Collect info about (dfa_state - char - nfa_states) transitions
            // in order to later convert it into (dfa_state - char - dfa_state) transitions
            curr_mapped_to.iter().for_each(|nfa_state| {
                if nfa.accept_states.contains(nfa_state) {
                    dfa.accept_states.insert(curr_state);
                }

                // SAFETY: nfa_state is guaranteed to be in nfa
                let nfa_trans = nfa.transitions.get(nfa_state).unwrap();

                nfa_trans.iter().for_each(|(symbol, nfa_to)| {
                    dfa_nfa_trans
                        .entry(*symbol)
                        .or_default()
                        .extend(nfa_to.iter());
                });
            });

            dfa_nfa_trans.iter().for_each(|(symbol, nfa_to)| {
                let dfa_to = match reverse_mapping.get(&nfa_to) {
                    Some(mapped_dfa) => *mapped_dfa,
                    None => {
                        let new_dfa = dfa.add_state();
                        mapping.insert(new_dfa, nfa_to.clone());
                        reverse_mapping.insert(nfa_to.clone(), new_dfa);
                        queue.push_back(new_dfa);
                        new_dfa
                    }
                };

                dfa.add_transition(curr_state, *symbol, dfa_to);
            });

            used.insert(curr_state);
        }

        dfa
    }

    pub fn to_full(&mut self) {
        let alphabet = self.get_alphabet();
        let drain = self.add_state();

        self.transitions
            .clone()
            .iter()
            .for_each(|(state, state_transitions)| {
                alphabet
                    .iter()
                    .filter(|symbol| state_transitions.get(symbol).is_none())
                    .for_each(|symbol| {
                        self.add_transition(*state, *symbol, drain);
                    });
            });
    }

    pub fn to_complement(&mut self) {
        self.accept_states = self
            .transitions
            .keys()
            .copied()
            .filter(|state| !self.accept_states.contains(state))
            .collect();
    }

    pub fn to_minimal(&mut self) {
        let mut queue = VecDeque::<(BTreeSet<AutomatonState>, AutomatonTransition)>::new();
        let allphabet = self.get_alphabet();
        let accept_class = self.accept_states.clone();
        let non_accept_class: BTreeSet<_> = self
            .transitions
            .keys()
            .copied()
            .filter(|state| !self.accept_states.contains(state))
            .collect();

        allphabet.iter().for_each(|sym| {
            queue.push_back((accept_class.clone(), *sym));
            queue.push_back((non_accept_class.clone(), *sym));
        });

        let mut partition =
            BTreeSet::<BTreeSet<AutomatonState>>::from([accept_class, non_accept_class]);

        while !queue.is_empty() {
            // SAFETY: queue is guaranteed not to be empty
            let (splitter, symbol) = queue.pop_front().unwrap();

            partition.clone().iter().for_each(|class| {
                let (splitter_reachable, splitter_unreachable): (BTreeSet<AutomatonState>, _) =
                    class.iter().partition(|state| {
                        // SAFETY: every state must have been created via
                        // new_state() and thus is present in transitions map
                        self.transitions
                            .get(*state)
                            .unwrap()
                            .get(&symbol)
                            .unwrap_or(&BTreeSet::<AutomatonState>::new())
                            .iter()
                            .filter(|dest_state| splitter.contains(*dest_state))
                            .next()
                            .is_some()
                    });

                if !splitter_reachable.is_empty() && !splitter_unreachable.is_empty() {
                    allphabet.iter().for_each(|sym| {
                        queue.push_back((splitter_reachable.clone(), *sym));
                        queue.push_back((splitter_unreachable.clone(), *sym));
                    });

                    partition.remove(class);
                    partition.insert(splitter_reachable);
                    partition.insert(splitter_unreachable);
                }
            });
        }

        let mut state_to_class_state = BTreeMap::<AutomatonState, AutomatonState>::new();
        let mut class_to_state = BTreeMap::<BTreeSet<AutomatonState>, AutomatonState>::new();

        partition.iter().for_each(|class| {
            let new_state = self.add_state();
            class_to_state.insert(class.clone(), new_state);

            class.iter().for_each(|state| {
                state_to_class_state.insert(*state, new_state);
            });
        });

        self.accept_states.clone().iter().for_each(|accept_state| {
            // SAFETY: every state must be in some equivalnce class
            // and every equivalnce class is mapped to some new state
            self.accept_states
                .insert(*state_to_class_state.get(accept_state).unwrap());
        });

        self.start_states.clone().iter().for_each(|start_state| {
            // SAFETY: every state must be in some equivalnce class
            // and every equivalnce class is mapped to some new state
            self.start_states
                .insert(*state_to_class_state.get(start_state).unwrap());
        });

        partition.iter().for_each(|class| {
            // SAFETY: all the class have been added to the map earlier
            let class_state = class_to_state.get(class).unwrap();

            class.iter().for_each(|old_state| {
                // SAFETY: every state must have been created via
                // new_state() and thus is present in transitions map
                self.transitions
                    .get(old_state)
                    .cloned()
                    .unwrap()
                    .iter()
                    .for_each(|(symbol, symbol_transitions)| {
                        symbol_transitions.iter().for_each(|symbol_transition| {
                            // SAFETY: every state must be in some equivalnce class
                            // and every equivalnce class is mapped to some new state
                            let class_transition =
                                state_to_class_state.get(symbol_transition).unwrap();

                            self.transitions
                                .entry(*class_state)
                                .or_default()
                                .entry(*symbol)
                                .or_default()
                                .insert(*class_transition);
                        });
                    });

                self.remove_state(*old_state);
            });
        });
    }

    pub fn get_alphabet(&self) -> AutomatonAlphabet {
        let mut alphabet = AutomatonAlphabet::new();

        self.transitions.values().for_each(|transition| {
            transition.keys().for_each(|symbol| {
                alphabet.insert(*symbol);
            })
        });

        alphabet
    }

    pub fn accepts_word(&self, word: &str) -> bool {
        let word = word.to_string();
        let mut curr_states = self.start_states.clone();

        for sym in word.chars() {
            let mut next_states = BTreeSet::<AutomatonState>::new();

            curr_states.iter().for_each(|state| {
                // SAFETY: every state must have been created via
                // new_state() and thus is present in transitions map
                let curr_trans = self.transitions.get(state).unwrap();

                if let Some(next) = curr_trans.get(&AutomatonTransition::Symbol(sym)) {
                    next_states.extend(next.iter());
                }
            });

            if next_states.is_empty() {
                return false;
            }

            curr_states = next_states;
        }

        curr_states
            .iter()
            .filter(|state| self.accept_states.contains(state))
            .next()
            .is_some()
    }

    pub fn dump(&self, file_name: &str) -> io::Result<()> {
        // SAFETY: 'G' is known to be a valid id string
        // SAFETY: all of the required fields of the graph are initialized
        let graph = GraphBuilder::default()
            .graph_type(GraphType::DiGraph)
            .strict(false)
            .id(Identity::id("G").unwrap())
            .stmts(self.build_graph())
            .build()
            .unwrap();

        let file = File::create(file_name)?;
        let mut writer = BufWriter::new(file);
        writeln!(&mut writer, "{}", graph)?;

        let png_file_name = file_name.to_owned() + ".png";
        Command::new("dot")
            .arg("-Tpng")
            .arg(file_name)
            .arg("-o")
            .arg(png_file_name)
            .spawn()?;

        Ok(())
    }

    fn build_graph(&self) -> StmtList {
        let mut stmt_list = StmtList::new();

        // Yes I have to loop all the states in advance in order to get the colors
        // right for them, because graphviz goes mad otherwise
        for state in self.transitions.keys() {
            let col = match self.accept_states.contains(state) {
                true => color(Color::Red),
                false => color(Color::Blue),
            };

            stmt_list = stmt_list
                .add_attr(AttrType::Node, AttrList::new().add_pair(col))
                .add_node(Identity::Usize(*state), None, None);
        }

        // An invisible mock state to draw arrows from to the start states
        let mock_state = -1;

        stmt_list = stmt_list
            .add_attr(
                AttrType::Node,
                AttrList::new()
                    .add_pair(shape(Shape::None))
                    .add_pair(label(""))
                    .add_pair(height(0_f64))
                    .add_pair(width(0_f64)),
            )
            .add_node(Identity::ISize(mock_state), None, None);

        // Add arrows to the start states
        for state in self.start_states.iter() {
            stmt_list = stmt_list.add_edge(
                Edge::head_node(Identity::ISize(mock_state), None)
                    .arrow_to_node(Identity::Usize(*state), None),
            );
        }

        for (from, transitions) in self.transitions.iter() {
            for (symbol, states) in transitions.iter() {
                let symbol = match symbol {
                    AutomatonTransition::Epsilon => '\u{03B5}',
                    AutomatonTransition::Symbol(sym) => *sym,
                };

                for to in states.iter() {
                    stmt_list = stmt_list.add_edge(
                        Edge::head_node(Identity::Usize(*from), None)
                            .arrow_to_node(Identity::Usize(*to), None)
                            .add_attrpair(label(char::to_string(&symbol))),
                    );
                }
            }
        }

        stmt_list
    }

    fn add_transition(
        &mut self,
        from: AutomatonState,
        symbol: AutomatonTransition,
        to: AutomatonState,
    ) {
        self.transitions
            .entry(from)
            .or_default()
            .entry(symbol)
            .or_default()
            .insert(to);
    }

    fn add_state(&mut self) -> AutomatonState {
        let new_state = self.last_state;
        self.last_state = self.last_state.saturating_add(1);
        self.transitions.insert(new_state, BTreeMap::new());
        new_state
    }

    fn remove_state(&mut self, state: AutomatonState) -> Option<AutomatonTransitionList> {
        self.start_states.remove(&state);
        self.accept_states.remove(&state);
        self.transitions.remove(&state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfa_to_dfa_unit_1() {
        let nfa = FiniteAutomaton {
            last_state: 3,
            start_states: BTreeSet::from([0]),
            accept_states: BTreeSet::from([2]),
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

        assert_eq!(dfa.start_states, BTreeSet::from([0]));
        assert_eq!(dfa.accept_states, BTreeSet::from([2]));
        assert_eq!(
            dfa.transitions,
            BTreeMap::from([
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
            ])
        )
    }

    #[test]
    fn nfa_to_dfa_unit_2() {
        let nfa = FiniteAutomaton {
            last_state: 3,
            start_states: BTreeSet::from([0]),
            accept_states: BTreeSet::from([2]),
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

        assert_eq!(dfa.start_states, BTreeSet::from([0]));
        assert_eq!(dfa.accept_states, BTreeSet::from([2]));
        assert_eq!(
            dfa.transitions,
            BTreeMap::from([
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
            ])
        );
    }

    #[test]
    fn nfa_to_dfa_unit_3() {
        let nfa = FiniteAutomaton {
            last_state: 6,
            start_states: BTreeSet::from([0]),
            accept_states: BTreeSet::from([5]),
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

        assert_eq!(dfa.start_states, BTreeSet::from([0]));
        assert_eq!(dfa.accept_states, BTreeSet::from([5, 6, 7, 8]));
        assert_eq!(
            dfa.transitions,
            BTreeMap::from([
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
            ])
        );
    }

    #[test]
    fn accepts_word_unit_1() {
        let regex = Regex::from_string("a((ba)*a(ab)* | a)*");
        let mut nfa = FiniteAutomaton::from_regex(&regex);
        nfa.eliminate_epsilon();

        assert_eq!(nfa.accepts_word("a"), true);
        assert_eq!(nfa.accepts_word("abaaa"), true);
        assert_eq!(nfa.accepts_word("abaabaab"), false);
        assert_eq!(nfa.accepts_word("ababab"), false);
        assert_eq!(nfa.accepts_word("abb"), false);

        let mut dfa = FiniteAutomaton::to_dfa(&nfa);
        dfa.to_full();
        dfa.to_minimal();

        assert_eq!(dfa.accepts_word("a"), true);
        assert_eq!(dfa.accepts_word("abaaa"), true);
        assert_eq!(dfa.accepts_word("abaabaab"), false);
        assert_eq!(dfa.accepts_word("ababab"), false);
        assert_eq!(dfa.accepts_word("abb"), false);
        }
}
