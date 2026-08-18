use std::cell::Cell;

// Global counter for unique identifiers

thread_local! {
    static TICK: Cell<u64> = const { Cell::new(0) };
}

pub fn refresh() {
    TICK.with(|tick| tick.set(0));
}

pub fn fresh() -> u64 {
    TICK.with(|tick| {
        let id = tick.get();
        tick.set(id + 1);
        id
    })
}
