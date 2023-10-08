use autore::{FiniteAutomaton, Regex};

fn main() -> std::io::Result<()> {
    let regex_initial = Regex::from_string("a((ba)*a(ab)* | a)*");
    regex_initial.dump("img/regex_initial.txt")?;

    let mut nfa = FiniteAutomaton::from_regex(&regex_initial);
    nfa.dump("img/nfa.dot")?;
    nfa.eliminate_epsilon();
    nfa.dump("img/nfa_without_epsilon.dot")?;

    let mut dfa = FiniteAutomaton::to_dfa(&nfa);
    dfa.dump("img/dfa.dot")?;

    dfa.make_full();
    dfa.dump("img/dfa_full.dot")?;

    dfa.make_minimal();
    dfa.dump("img/dfa_minimal.dot")?;

    dfa.make_complement();
    dfa.dump("img/dfa_complement.dot")?;

    dfa.make_complement();
    let final_regex = Regex::from_finite_automaton(&dfa);
    final_regex.dump("img/regex_final.txt")?;

    let mut nfa_second = FiniteAutomaton::from_regex(&final_regex);
    nfa_second.eliminate_epsilon();
    let mut dfa_second = FiniteAutomaton::to_dfa(&nfa);
    dfa_second.make_full();
    dfa_second.make_minimal();
    dfa_second.dump("img/dfa_second_minimal.dot")?;

    Ok(())
}
