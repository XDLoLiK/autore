release:
	cargo build --release

debug:
	cargo build --debug

%.png: %.dot
	dot -Tpng $^ -o $@

coverage: 
	rustup component add llvm-tools-preview
	export RUSTFLAGS="-Cinstrument-coverage"
	export LLVM_PROFILE_FILE="autore-%p-%m.profraw"
	cargo build
	cargo test
	grcov . -s . --binary-path ./target/debug/ -t html --branch --ignore-not-existing -o ./target/debug/coverage/

clean:
	rm -rf *.profraw img/*

