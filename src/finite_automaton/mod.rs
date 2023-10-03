use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    fs::File,
    io::{BufWriter, Result, Write},
    ops::Deref,
    process::Command,
};

use tabbycat::attributes::*;
use tabbycat::{AttrList, AttrType, Edge, GraphBuilder, GraphType, Identity, StmtList};

use super::{
    AutomatonKind, AutomatonState, AutomatonTransition, FiniteAutomaton, Regex, RegexEntry,
    RegexOps,
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
        let mut closure_matrix =
            BTreeMap::<AutomatonState, BTreeMap<AutomatonState, bool>>::default();

        for (curr_state, transitions) in self.transitions.iter() {
            if let Some(epsilon_trans) = transitions.get(&AutomatonTransition::Epsilon) {
                for epsilon_state in epsilon_trans.iter() {
                    let reachable = closure_matrix.entry(*curr_state).or_default();
                    reachable.insert(*epsilon_state, true);
                }
            }
        }

        self.floyd_warshall(&mut closure_matrix);

        for (from, epsilon_trans) in closure_matrix.iter() {
            for (to, reachable) in epsilon_trans.iter() {
                if *reachable {
                    self.add_transition(*from, AutomatonTransition::Epsilon, *to);
                }
            }
        }

        // Step 2: Update accept states
        for (curr_state, transitions) in self.transitions.iter() {
            if let Some(epsilon_trans) = transitions.get(&AutomatonTransition::Epsilon) {
                for epsilon_state in epsilon_trans.iter() {
                    if self.accept_states.contains(epsilon_state) {
                        self.accept_states.insert(*curr_state);
                    }
                }
            }
        }

        // Step 3: Add (u - Epsilon - v - Symbol - w) edges
        let cloned_transitions = self.transitions.clone();

        for (curr_state, transitions) in cloned_transitions.iter() {
            if let Some(epsilon_trans) = transitions.get(&AutomatonTransition::Epsilon) {
                for epsilon_state in epsilon_trans.iter() {
                    // SAFETY: every state must have been created via
                    // new_state() and thus is present in transitions map
                    let next_trans = cloned_transitions.get(epsilon_state).unwrap();

                    for (symbol, dest_list) in next_trans.iter() {
                        for dest_state in dest_list.iter() {
                            self.add_transition(*curr_state, *symbol, *dest_state);
                        }
                    }
                }
            }
        }

        // Step 4: Remove all the epsilon transitions from the automaton
        for (_, transitions) in self.transitions.iter_mut() {
            transitions.remove(&AutomatonTransition::Epsilon);
        }

        // Step 5: Remove the dead states that could have appeared
        self.eliminate_dead();
        self.kind = AutomatonKind::NfaWithoutEpsilon;
    }

    fn floyd_warshall(
        &self,
        matrix: &mut BTreeMap<AutomatonState, BTreeMap<AutomatonState, bool>>,
    ) {
        for (k, _) in self.transitions.iter() {
            for (i, _) in self.transitions.iter() {
                for (j, _) in self.transitions.iter() {
                    let matrix_i_k = *matrix.entry(*i).or_default().entry(*k).or_default();
                    let matrix_k_j = *matrix.entry(*k).or_default().entry(*j).or_default();
                    let entry_i_j = matrix.entry(*i).or_default().entry(*j).or_default();
                    *entry_i_j = *entry_i_j | (matrix_i_k & matrix_k_j);
                }
            }
        }
    }

    pub fn eliminate_dead(&mut self) {
        let mut ref_count = BTreeMap::<AutomatonState, usize>::default();

        for (_, transitions) in self.transitions.iter() {
            for (_, states) in transitions.iter() {
                for state in states.iter() {
                    *ref_count.entry(*state).or_default() += 1;
                }
            }
        }

        for (state, _) in self.transitions.clone().iter() {
            // We never want to eliminate state 0 here because
            // we would then leave no start states at all
            if *state != 0 && *ref_count.entry(*state).or_default() == 0 {
                self.remove_state(*state);
            }
        }
    }

    pub fn to_dfa(nfa: &FiniteAutomaton) -> Self {
        if nfa.transitions.is_empty() || nfa.kind != AutomatonKind::NfaWithoutEpsilon {
            return Self::default();
        }

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
            for nfa_state in curr_mapped_to.iter() {
                if nfa.accept_states.contains(nfa_state) {
                    dfa.accept_states.insert(curr_state);
                }

                // SAFETY: nfa_state is guaranteed to be in nfa
                let nfa_trans = nfa.transitions.get(nfa_state).unwrap();

                for (symbol, nfa_to) in nfa_trans.iter() {
                    let entry = dfa_nfa_trans.entry(*symbol).or_default();
                    entry.extend(nfa_to.iter());
                }
            }

            for (symbol, nfa_to) in dfa_nfa_trans.iter() {
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
            }

            used.insert(curr_state);
        }

        dfa.kind = AutomatonKind::Dfa;
        dfa
    }

    pub fn to_full(&mut self) {
        if self.kind != AutomatonKind::Dfa {
            return;
        }

        let mut alphabet = BTreeSet::<AutomatonTransition>::default();

        for (_, transitions) in self.transitions.iter() {
            for (symbol, _) in transitions.iter() {
                alphabet.insert(*symbol);
            }
        }

        let drain = self.add_state();

        for (state, transitions) in self.transitions.clone().iter() {
            for symbol in alphabet.iter() {
                if let None = transitions.get(symbol) {
                    self.add_transition(*state, *symbol, drain);
                }
            }
        }

        self.kind = AutomatonKind::FullDfa;
    }

    pub fn to_complement(&mut self) {
        if self.kind != AutomatonKind::FullDfa {
            return;
        }

        for (curr_state, _) in self.transitions.iter() {
            if self.accept_states.contains(curr_state) {
                self.accept_states.remove(curr_state);
            } else {
                self.accept_states.insert(*curr_state);
            }
        }
    }

    pub fn accepts_word(&self, word: &str) -> bool {
        let word = word.to_string();
        let mut accepts = true;
        let mut curr_states = self.start_states.clone();

        for sym in word.chars() {
            let mut next_states = BTreeSet::<AutomatonState>::default();

            for state in curr_states.iter() {
                // SAFETY: every state must have been created via
                // new_state() and thus is present in transitions map
                let curr_trans = self.transitions.get(state).unwrap();

                if let Some(next) = curr_trans.get(&AutomatonTransition::Symbol(sym)) {
                    next_states.extend(next.iter());
                }
            }

            if next_states.is_empty() {
                accepts = false;
                break;
            }

            curr_states = next_states;
        }

        accepts
    }

    pub fn dump(&self, file_name: &str) -> Result<()> {
        // SAFETY: G is known to be a valid lexem
        // SAFETY: all the required fields of the graph are initialized
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

        // FIXME: command doesn't execute properly
        if cfg!(target_os = "linux") {
            Command::new("dot")
                .arg("-Tpng")
                .arg(file_name)
                .arg("-o")
                .arg(png_file_name)
                .output()
                .expect("Failed to execute the process");
        }

        Ok(())
    }

    fn build_graph(&self) -> StmtList {
        let mut stmt_list = StmtList::new();

        // Yes I have to loop all the states in advance in order to get the colors
        // right for them, because graphviz goes mad otherwise
        for (from, _) in self.transitions.iter() {
            let col = match self.accept_states.contains(from) {
                true => color(Color::Red),
                false => color(Color::Blue),
            };

            stmt_list = stmt_list
                .add_attr(AttrType::Node, AttrList::new().add_pair(col))
                .add_node(Identity::Usize(*from), None, None);
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
        src: AutomatonState,
        trans: AutomatonTransition,
        dest: AutomatonState,
    ) {
        let trans_list = self.transitions.entry(src).or_default();
        let dest_list = trans_list.entry(trans).or_default();
        dest_list.insert(dest);
    }

    fn add_state(&mut self) -> AutomatonState {
        let new_state = self.last_state;
        self.last_state += 1;
        self.transitions.insert(new_state, BTreeMap::default());
        new_state
    }

    fn remove_state(&mut self, state: AutomatonState) {
        self.start_states.remove(&state);
        self.accept_states.remove(&state);
        self.transitions.remove(&state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfa_to_dfa_unit_1() {
        let nfa = FiniteAutomaton {
            last_state: 3,
            kind: AutomatonKind::NfaWithoutEpsilon,
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
            kind: AutomatonKind::NfaWithoutEpsilon,
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
            kind: AutomatonKind::NfaWithoutEpsilon,
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
}
