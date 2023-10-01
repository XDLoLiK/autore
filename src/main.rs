use autore::{FiniteAutomaton, Regex};

fn main() {
    let regex = Regex::from_string("a((ba)*a(ab)* | a)*");
    println!("{:#?}", regex);

    let mut nfa = FiniteAutomaton::from_regex(&regex);
    nfa.dump("img/nfa.dot");
    nfa.eliminate_epsilon();
    nfa.dump("img/nfa_without_epsilon.dot");

    let mut dfa = FiniteAutomaton::to_dfa(&nfa);
    dfa.dump("img/dfa.dot");

    dfa.to_full();
    dfa.dump("img/dfa_full.dot");

    dfa.to_complement();
    dfa.dump("img/dfa_complement.dot");
}
