use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs::File,
    io::{self, BufReader, BufWriter, Read, Write},
    ops::Deref,
};

use colored::Colorize;

use super::{AutomatonState, AutomatonTransition, FiniteAutomaton, Regex, RegexEntry, RegexOps};

#[derive(Debug, Default, Clone)]
struct RegexParser {
    expr: String,
    curr_pos: usize,
}

impl Regex {
    pub fn from_string(expr: &str) -> Self {
        let mut regex_parser = RegexParser::new(expr.to_string());
        regex_parser.get_regex()
    }

    pub fn from_file(file: &File) -> Self {
        let mut reader = BufReader::new(file);
        let mut expr = String::new();

        if let Err(error) = reader.read_to_string(&mut expr) {
            panic!("Error when reading from file: {:#?}", error);
        }

        let mut regex_parser = RegexParser::new(expr);
        regex_parser.get_regex()
    }

    pub fn from_finite_automaton(automaton: &FiniteAutomaton) -> Self {
        let mut automaton = automaton.clone();

        // Step 1: Make sure there's only one start state
        // and there are no incoming edges to it
        let new_start = automaton.add_state();

        automaton
            .start_states
            .clone()
            .iter()
            .for_each(|start_state| {
                automaton.add_transition(new_start, AutomatonTransition::Epsilon, *start_state);
            });

        automaton.start_states.clear();
        automaton.start_states.insert(new_start);

        // Step 2: Make sure there's only one accept state
        // and there are no outcoming edges from it
        let new_accept = automaton.add_state();

        automaton
            .accept_states
            .clone()
            .iter()
            .for_each(|accept_state| {
                automaton.add_transition(*accept_state, AutomatonTransition::Epsilon, new_accept);
            });

        automaton.accept_states.clear();
        automaton.accept_states.insert(new_accept);

        // Step 3: Eliminate all the intermediate states one by one
        let mut regular_transitions =
            BTreeMap::<AutomatonState, BTreeMap<AutomatonState, RegexEntry>>::new();

        let mut reverse_regular_transitions =
            BTreeMap::<AutomatonState, BTreeMap<AutomatonState, RegexEntry>>::new();

        automaton
            .transitions
            .iter()
            .for_each(|(state, state_transitions)| {
                state_transitions
                    .iter()
                    .for_each(|(symbol, symbol_transitions)| {
                        symbol_transitions.iter().for_each(|symbol_transition| {
                            Self::add_regular_transition(
                                &mut regular_transitions,
                                *state,
                                Self::symbol_to_regex_ops(*symbol),
                                *symbol_transition,
                            );

                            Self::add_regular_transition(
                                &mut reverse_regular_transitions,
                                *symbol_transition,
                                Self::symbol_to_regex_ops(*symbol),
                                *state,
                            );
                        });
                    });
            });

        let mut used = BTreeSet::<AutomatonState>::new();
        let mut queue = VecDeque::<AutomatonState>::from([new_start]);

        while !queue.is_empty() {
            // SAFETY: queue is guaranteed not to be empty
            let curr_state = queue.pop_front().unwrap();

            if used.contains(&curr_state) {
                continue;
            }

            let self_transition = regular_transitions
                .entry(curr_state)
                .or_default()
                .get(&curr_state)
                .cloned();

            let outcoming = regular_transitions.remove(&curr_state).unwrap_or_default();
            let incoming = reverse_regular_transitions
                .remove(&curr_state)
                .unwrap_or_default();

            outcoming
                .iter()
                .filter(|(to, _)| **to != curr_state)
                .for_each(|(to, to_regex)| {
                    incoming
                        .iter()
                        .filter(|(from, _)| **from != curr_state)
                        .for_each(|(from, from_regex)| {
                            let regex_combined = match &self_transition {
                                Some(self_transition) => Box::new(RegexOps::Consecutive(
                                    Box::new(RegexOps::Consecutive(
                                        from_regex.clone(),
                                        Box::new(RegexOps::NoneOrMore(self_transition.clone())),
                                    )),
                                    to_regex.clone(),
                                )),
                                None => Box::new(RegexOps::Consecutive(
                                    from_regex.clone(),
                                    to_regex.clone(),
                                )),
                            };

                            Self::add_regular_transition(
                                &mut regular_transitions,
                                *from,
                                regex_combined.clone(),
                                *to,
                            );

                            Self::add_regular_transition(
                                &mut reverse_regular_transitions,
                                *to,
                                regex_combined.clone(),
                                *from,
                            );
                        });
                });

            incoming.keys().for_each(|from| queue.push_back(*from));
            outcoming.keys().for_each(|to| queue.push_back(*to));
            used.insert(curr_state);
        }

        // SAFETY: new_start is guaranteed to be present in the map
        Self {
            root: regular_transitions
                .get(&new_start)
                .unwrap()
                .get(&new_accept)
                .cloned(),
        }
    }

    fn symbol_to_regex_ops(symbol: AutomatonTransition) -> RegexEntry {
        match symbol {
            AutomatonTransition::Epsilon => Box::new(RegexOps::Epsilon),
            AutomatonTransition::Symbol(symbol) => Box::new(RegexOps::Symbol(symbol)),
        }
    }

    fn add_regular_transition(
        regular_transitions: &mut BTreeMap<AutomatonState, BTreeMap<AutomatonState, RegexEntry>>,
        from: AutomatonState,
        regex: RegexEntry,
        to: AutomatonState,
    ) {
        regular_transitions
            .entry(from)
            .or_default()
            .entry(to)
            .and_modify(|regex_entry| {
                *regex_entry = Box::new(RegexOps::Either(regex.clone(), regex_entry.clone()))
            })
            .or_insert(regex.clone());
    }

    pub fn dump(&self, file_name: &str) -> io::Result<()> {
        let file = File::create(file_name)?;
        let mut writer = BufWriter::new(file);

        if let Some(root) = &self.root {
            Self::dump_helper(root, &mut writer)?;
        }

        Ok(())
    }

    fn dump_helper(curr_node: &RegexEntry, writer: &mut BufWriter<File>) -> io::Result<()> {
        match curr_node.deref() {
            RegexOps::Either(left, right) => {
                Self::dump_helper(left, writer)?;
                write!(writer, " | ")?;
                Self::dump_helper(right, writer)?;
            }
            RegexOps::Consecutive(left, right) => {
                Self::dump_helper(left, writer)?;
                Self::dump_helper(right, writer)?;
            }
            RegexOps::NoneOrMore(what) => {
                write!(writer, "(")?;
                Self::dump_helper(what, writer)?;
                write!(writer, ")*")?;
            }
            RegexOps::NoneOrOnce(what) => {
                write!(writer, "(")?;
                Self::dump_helper(what, writer)?;
                write!(writer, ")?")?;
            }
            RegexOps::OnceOrMore(what) => {
                write!(writer, "(")?;
                Self::dump_helper(what, writer)?;
                write!(writer, ")+")?;
            }
            RegexOps::Symbol(symbol) => {
                write!(writer, "{}", symbol)?;
            }
            RegexOps::Epsilon => {
                write!(writer, "{}", '\u{03B5}')?;
            }
        };

        Ok(())
    }
}

impl RegexParser {
    fn new(mut expr: String) -> Self {
        expr.retain(|sym| !sym.is_whitespace());
        Self { expr, curr_pos: 0 }
    }

    fn get_regex(&mut self) -> Regex {
        Regex {
            root: Some(self.parse_either()),
        }
    }

    fn parse_either(&mut self) -> RegexEntry {
        let mut left = self.parse_consecutive();

        while let Some('|') = self.expr.chars().nth(self.curr_pos) {
            self.curr_pos += 1;
            let right = self.parse_consecutive();
            left = Box::new(RegexOps::Either(left, right));
        }

        left
    }

    fn parse_consecutive(&mut self) -> RegexEntry {
        let mut left = self.parse_repeat();

        while let Some(symbol) = self.expr.chars().nth(self.curr_pos) {
            // Only alhabetic characters and left paranthesis are valid options
            if !(symbol.is_alphabetic() || symbol == '(') {
                break;
            }

            let right = self.parse_repeat();
            left = Box::new(RegexOps::Consecutive(left, right));
        }

        left
    }

    fn parse_repeat(&mut self) -> RegexEntry {
        let mut ret = self.parse_priority();

        while let Some(symbol) = self.expr.chars().nth(self.curr_pos) {
            match symbol {
                '*' => ret = Box::new(RegexOps::NoneOrMore(ret)),
                '?' => ret = Box::new(RegexOps::NoneOrOnce(ret)),
                '+' => ret = Box::new(RegexOps::OnceOrMore(ret)),
                _ => break,
            }

            self.curr_pos += 1;
        }

        ret
    }

    fn parse_priority(&mut self) -> RegexEntry {
        match self.expr.chars().nth(self.curr_pos) {
            Some('(') => {
                self.curr_pos += 1;
                let ret = self.parse_either();

                match self.expr.chars().nth(self.curr_pos) {
                    Some(')') => self.curr_pos += 1,
                    _ => self.report_error("')' expected"),
                }

                ret
            }
            _ => self.parse_symbol(),
        }
    }

    fn parse_symbol(&mut self) -> RegexEntry {
        match self.expr.chars().nth(self.curr_pos) {
            Some('1') => {
                self.curr_pos += 1;
                Box::new(RegexOps::Epsilon)
            }
            Some(symbol) => {
                self.curr_pos += 1;
                Box::new(RegexOps::Symbol(symbol))
            }
            None => {
                self.report_error("unexpected end of the expression");
            }
        }
    }

    fn report_error(&self, error_msg: &str) -> ! {
        let expr_str = self.expr.as_str();
        let curr_pos = self.curr_pos;
        panic!(
            "Parser error ({}): {}{}{}",
            error_msg,
            &expr_str[..curr_pos],
            &expr_str[curr_pos..(curr_pos + 1)].red(),
            &expr_str[(curr_pos + 1)..]
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_string_unit_1() {
        let regex = Regex::from_string("(a|b)*ab");

        assert_eq!(
            regex,
            Regex {
                root: Some(Box::new(RegexOps::Consecutive(
                    Box::new(RegexOps::Consecutive(
                        Box::new(RegexOps::NoneOrMore(Box::new(RegexOps::Either(
                            Box::new(RegexOps::Symbol('a')),
                            Box::new(RegexOps::Symbol('b'))
                        )))),
                        Box::new(RegexOps::Symbol('a'))
                    )),
                    Box::new(RegexOps::Symbol('b'))
                ))),
            }
        );
    }
}
