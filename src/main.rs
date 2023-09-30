use autore::{FiniteAutomaton, Regex};

fn main() {
    let regex = Regex::from_string("a+b");
    let nfa = FiniteAutomaton::from_regex(&regex);
    let dfa = FiniteAutomaton::to_dfa(&nfa);

    println!("{:#?}", regex);
    println!("{:#?}", nfa);
    println!("{:#?}", dfa);
}
