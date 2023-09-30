use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    fs::File,
    io::{BufWriter, Write},
    ops::Deref,
    process::Command,
};

use tabbycat::attributes::*;
use tabbycat::{AttrList, Edge, GraphBuilder, GraphType, Identity, StmtList};

use super::{
    AutomatonState, AutomatonTransition, AutomatonTransitionList, FiniteAutomaton, Regex,
    RegexEntry, RegexOps,
};

impl FiniteAutomaton {
    pub fn from_regex(regex: &Regex) -> Self {
        match regex.root.as_ref() {
            Some(root) => {
                let mut nfa = Self::default();
                let start_state = nfa.new_state();
                nfa.start_states = BTreeSet::from([start_state]);
                let accept_state = nfa.new_state();
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
        if self.transitions.is_empty() {
            return;
        }

        let mut used = HashSet::<AutomatonState>::new();
        let mut queue = VecDeque::<AutomatonState>::new();
        queue.extend(self.start_states.iter());

        while !queue.is_empty() {
            // It is safe unwrap here as queue is guaranteed not to be empty
            let curr_state = queue.pop_front().unwrap();

            if used.contains(&curr_state) {
                continue;
            }

            // It is safe to unwrap here because every state must have been created via
            // new_state() and thus is present in transitions map
            let epsilon_trans = self
                .transitions
                .get_mut(&curr_state)
                .unwrap()
                .remove(&AutomatonTransition::Epsilon);

            if epsilon_trans.is_none() {
                // It is safe to unwrap here because every state must have been created via
                // new_state() and thus is present in transitions map
                for (_, states) in self.transitions.get(&curr_state).unwrap() {
                    queue.extend(states.iter());
                }

                used.insert(curr_state);
                continue;
            }

            // It safe to unwrap here because we checked epsilon_trans for None earlier
            let epsilon_trans = epsilon_trans.unwrap();
            let mut add_trans = Vec::<AutomatonTransitionList>::new();

            for state in epsilon_trans.iter() {
                // It is safe to unwrap here because every state must have been created via
                // new_state() and thus is present in transitions map
                let state_trans = self.transitions.get_mut(state).cloned().unwrap();
                add_trans.push(state_trans);

                if self.start_states.contains(&curr_state) {
                    self.start_states.insert(*state);
                }

                if self.accept_states.contains(state) {
                    self.accept_states.insert(curr_state);
                }
            }

            let curr_trans = self.transitions.get_mut(&curr_state).unwrap();

            for mut trans in add_trans {
                curr_trans.append(&mut trans);
            }

            // We could have added more epsilon transitions here, so reschedule ourselves
            queue.push_front(curr_state);
        }

        self.eliminate_dead();
    }

    pub fn eliminate_dead(&mut self) {
        let states_nr = self.transitions.len();
        let mut ref_count = vec![0; states_nr];

        for (_, transitions) in self.transitions.iter() {
            for (_, states) in transitions.iter() {
                for state in states.iter() {
                    ref_count[*state] += 1;
                }
            }
        }

        // We never want to eliminate state 0 here because
        for state in 1..ref_count.len() {
            if ref_count[state] == 0 {
                self.start_states.remove(&state);
                self.accept_states.remove(&state);
                self.transitions.remove(&state);
            }
        }
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

        let start_state = dfa.new_state();
        dfa.start_states = BTreeSet::from([start_state]);
        queue.push_back(start_state);
        mapping.insert(start_state, nfa.start_states.clone());
        reverse_mapping.insert(nfa.start_states.clone(), start_state);

        while !queue.is_empty() {
            // It is safe to unwrap here as queue is guaranteed not to be empty
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
                    entry.extend(nfa_to.iter());
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

    pub fn dump(&self, file_name: &str) {
        let graph = GraphBuilder::default()
            .graph_type(GraphType::DiGraph)
            .strict(false)
            .id(Identity::id("G").unwrap())
            .stmts(self.build_graph())
            .build()
            .unwrap();

        let file = File::create(file_name).unwrap();
        let mut writer = BufWriter::new(file);
        writeln!(&mut writer, "{}", graph).unwrap();
        let png_file_name = file_name.to_owned() + ".png";

        // FIXME: command doesn't execute properly
        if cfg!(target_os = "linux") {
            Command::new("dot")
                .arg("-Tpng")
                .arg(file_name)
                .arg("-o")
                .arg(png_file_name)
                .output()
                .expect("failed to execute the process");
        }
    }

    fn build_graph(&self) -> StmtList {
        let mut stmt_list = StmtList::default();

        // Yes I have to loop all the states in advance in order to get the colors
        // right for them, because graphviz goes mad otherwise
        for (from, _) in self.transitions.iter() {
            let col = match self.start_states.contains(from) {
                true => color(Color::Green),
                false => match self.accept_states.contains(from) {
                    true => color(Color::Red),
                    false => color(Color::Blue),
                },
            };

            stmt_list = stmt_list
                .add_attr(tabbycat::AttrType::Node, AttrList::default().add_pair(col))
                .add_node(Identity::Usize(*from), None, None);
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
                            .add_attrpair(arrowhead(ArrowShape::Diamond))
                            .add_attrpair(label(char::to_string(&symbol))),
                    );
                }
            }
        }

        stmt_list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfa_to_dfa_unit_1() {
        let nfa = FiniteAutomaton {
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

        assert_eq!(
            dfa,
            FiniteAutomaton {
                start_states: BTreeSet::from([0]),
                accept_states: BTreeSet::from([2]),
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

        assert_eq!(
            dfa,
            FiniteAutomaton {
                start_states: BTreeSet::from([0]),
                accept_states: BTreeSet::from([2]),
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

        assert_eq!(
            dfa,
            FiniteAutomaton {
                start_states: BTreeSet::from([0]),
                accept_states: BTreeSet::from([5, 6, 7, 8]),
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
