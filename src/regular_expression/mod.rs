use std::{
    fs::File,
    io::{BufReader, Read},
};

use super::{Regex, RegexEntry, RegexOps};

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

        while let Some('+') = self.expr.chars().nth(self.curr_pos) {
            self.curr_pos += 1;
            let right = self.parse_consecutive();
            left = Box::new(RegexOps::Either(left, right));
        }

        left
    }

    fn parse_consecutive(&mut self) -> RegexEntry {
        let mut left = self.parse_repeat();

        while let Some(symbol) = self.expr.chars().nth(self.curr_pos) {
            if !symbol.is_alphabetic() {
                break;
            }

            let right = self.parse_repeat();
            left = Box::new(RegexOps::Consecutive(left, right));
        }

        left
    }

    fn parse_repeat(&mut self) -> RegexEntry {
        let mut ret = self.parse_priority();

        while let Some('*') = self.expr.chars().nth(self.curr_pos) {
            self.curr_pos += 1;
            ret = Box::new(RegexOps::Repeat(ret));
        }

        ret
    }

    fn parse_priority(&mut self) -> RegexEntry {
        match self.expr.chars().nth(self.curr_pos) {
            Some('(') => {
                self.curr_pos += 1;
                let ret = self.parse_either();
                assert_eq!(self.expr.chars().nth(self.curr_pos), Some(')'));
                self.curr_pos += 1;
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
                panic!("Parser error: unexpected end of string");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_string_unit_1() {
        let regex = Regex::from_string("(a + b)*ab");

        assert_eq!(
            regex,
            Regex {
                root: Some(Box::new(RegexOps::Consecutive(
                    Box::new(RegexOps::Consecutive(
                        Box::new(RegexOps::Repeat(Box::new(RegexOps::Either(
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
