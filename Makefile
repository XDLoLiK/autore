release:
	cargo build --release

debug:
	cargo build --debug

clean:
	cargo clean
	rm -rf *.profraw img/*

