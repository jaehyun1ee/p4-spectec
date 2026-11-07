# Small P4 and p4st-in-X specifications, isolated from the main implementation
#
# make metameta   mini P4 under meta- and meta-meta-execution (p4st-in-p4st), compared
# make redex      the walk-through on mini p4st-in-redex, with its trace
# make k          the walk-through on mini p4st-in-k
# make k-trace    the <k> cell around the first UNMATCH of the K walk-through
# make p4-typecheck  ebpf.p4 and filter.p4 through the full P4 specification
# make all        metameta, redex, and k

DRIVER = driver/target/release/metasq-example
P4SPECTEC = ../p4spectec/target/release/p4spectec
OUT = out

.PHONY: all driver meta metameta emit redex k k-trace p4-typecheck clean

all: metameta redex k

driver:
	cargo build --locked --release --manifest-path driver/Cargo.toml

meta: driver
	$(DRIVER) meta

metameta: driver
	$(DRIVER) metameta

emit: driver
	mkdir -p $(OUT)
	$(DRIVER) emit $(OUT)

redex: emit
	racket st-in-redex/run.rkt $(OUT)/walkthrough.rktd

st-in-k/st-kompiled/timestamp: st-in-k/st.k
	kompile st-in-k/st.k --main-module ST --syntax-module ST-SYNTAX --backend llvm -o st-in-k/st-kompiled

k: emit st-in-k/st-kompiled/timestamp
	krun --definition st-in-k/st-kompiled $(OUT)/walkthrough.k | grep -A1 '<k>'

# The depths bracket the first UNMATCH of rule if_true and the move to if_false
k-trace: emit st-in-k/st-kompiled/timestamp
	python3 -I st-in-k/trace.py st-in-k/st-kompiled $(OUT)/walkthrough.k 0 3
	python3 -I st-in-k/trace.py st-in-k/st-kompiled $(OUT)/walkthrough.k 789 797

p4-typecheck:
	cargo build --locked --release --manifest-path ../p4spectec/Cargo.toml --bin p4spectec
	$(P4SPECTEC) run --rel Program_ok -i p4/program -p p4/program/filter.p4 $$(find ../spec -name '*.watsup' | sort)

clean:
	rm -rf $(OUT) st-in-k/st-kompiled driver/target
