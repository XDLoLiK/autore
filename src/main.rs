use autore::{FiniteAutomaton, Regex};

fn main() {
    let regex = Regex::from_string("a((ba)*a(ab)* | a)*");
    println!("{:#?}", regex);

    let mut nfa = FiniteAutomaton::from_regex(&regex);
    nfa.dump("nfa.dot");
    nfa.eliminate_epsilon();
    nfa.dump("nfa_without_epsilon.dot");

    let dfa = FiniteAutomaton::to_dfa(&nfa);
    dfa.dump("dfa.dot");
}
