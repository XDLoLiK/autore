# autore
A tiny library for converting regular expressions to different kinds of finite automatons and vise versa

# dependecies
- make
- rust (edition 2023)
- graphviz
- colored (version 2.0.4)
- tabbycat (version 0.1.3)

# building
Run either of these in order to build the release version
```
$ make release
$ cargo build --release
```
To get png file from the corresponding dot file run the following command
```
$ make img/dfa.png
```

# external links
NFA epsilon transitions elimination:
- https://shorturl.at/gjpt7
- https://www.geeksforgeeks.org/conversion-of-epsilon-nfa-to-nfa/

NFA from regular expression:
- https://www.tutorialspoint.com/what-is-the-conversion-of-a-regular-expression-to-finite-automata-nfa

NFA to DFA conversion:
- https://www.geeksforgeeks.org/conversion-from-nfa-to-dfa/
