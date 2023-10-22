use std::io;

use autore::{min_word_len_exectly_symbol_count, FiniteAutomaton, Regex};

fn main() -> io::Result<()> {
    let mut rpn = String::new();
    io::stdin().read_line(&mut rpn)?;
    let regex = Regex::from_rpn(&rpn);

    let mut nfa = FiniteAutomaton::from_regex(&regex);
    nfa.eliminate_epsilon();

    // 11
    let read_x_k = || -> (char, usize) {
        let mut buf = String::new();
        io::stdin().read_line(&mut buf).unwrap();
        let mut x_k = buf.trim().split_whitespace();

        (
            x_k.next().unwrap().parse::<char>().unwrap(),
            x_k.next().unwrap().parse::<usize>().unwrap(),
        )
    };

    let (x, k) = read_x_k();
    let (is_found, min_len) = min_word_len_exectly_symbol_count(&nfa, x, k);

    if is_found {
        println!("{min_len}");
    } else {
        println!("INF");
    }

    Ok(())
}
