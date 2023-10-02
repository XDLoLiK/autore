release:
	cargo build --release

debug:
	cargo build --debug

%.png: %.dot
	dot -Tpng $^ -o $@

clean:
	rm -rf *.profraw img/*

