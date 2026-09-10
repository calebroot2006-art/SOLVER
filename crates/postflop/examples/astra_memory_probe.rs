//! Review-only allocation counter for step 6 construction.
use cards::{Card, Range};
use postflop::{
    Precision,
    streets::{PostflopGame, PostflopMemory, PostflopOptions},
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use tree::{BetSizeOptions, PostflopTree, PostflopTreeConfig, Street};

struct Counted;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn added(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Ordering::SeqCst) + bytes;
    PEAK.fetch_max(live, Ordering::SeqCst);
}

// Delegates unchanged layouts and pointers to System. Atomics allocate nothing.
unsafe impl GlobalAlloc for Counted {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            added(layout.size());
        }
        ptr
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            added(layout.size());
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::SeqCst);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, size) };
        if !new_ptr.is_null() {
            if size >= layout.size() {
                added(size - layout.size());
            } else {
                LIVE.fetch_sub(layout.size() - size, Ordering::SeqCst);
            }
        }
        new_ptr
    }
}

#[global_allocator]
static ALLOCATOR: Counted = Counted;

fn menu(bets: &str, raises: &str) -> [BetSizeOptions; 2] {
    let sizes = BetSizeOptions::try_from((bets, raises)).unwrap();
    [sizes.clone(), sizes]
}

fn main() {
    let tree = PostflopTree::new(PostflopTreeConfig {
        starting_pot: 55,
        effective_stack: 975,
        min_bet: 10,
        start_street: Street::Flop,
        sizes: [
            menu("33%,a", "100%,a"),
            menu("33%,a", "100%,a"),
            menu("33%,75%", "100%,a"),
        ],
        max_raises: 1,
        add_all_in_threshold: 0.0,
        force_all_in_threshold: 0.0,
        max_nodes: 100_000,
    })
    .unwrap();
    let board: Vec<Card> = "9c 5d 2h"
        .split_ascii_whitespace()
        .map(|s| s.parse().unwrap())
        .collect();
    let ranges = [Range::parse("AcAd").unwrap(), Range::parse("KcKd").unwrap()];
    let estimate = PostflopMemory::for_tree_over(&tree, 3, 1, [1, 1]).unwrap();
    let bound = estimate.working_set_bound_bytes;
    println!(
        "sparse flop: nodes={}, bound={bound}, shared={}, construction={}",
        estimate.expanded_nodes, estimate.shared_bytes, estimate.construction_bytes
    );
    // Excludes all allocations made before construction, including the compact
    // input tree. Thus the observed increment is a lower bound on the full job.
    let before = LIVE.load(Ordering::SeqCst);
    PEAK.store(before, Ordering::SeqCst);
    let game = PostflopGame::new(
        &board,
        ranges,
        tree,
        PostflopOptions {
            memory_limit_bytes: bound,
            precision: Precision::F64,
            threads: 1,
        },
    )
    .unwrap();
    let peak_increment = PEAK.load(Ordering::SeqCst) - before;
    let retained_increment = LIVE.load(Ordering::SeqCst) - before;
    println!(
        "construction: incremental_peak={peak_increment}, incremental_retained={retained_increment}, reserved={}, limit={bound}",
        game.reserved_bytes()
    );
    assert!(
        peak_increment <= bound,
        "construction alone exceeded the complete admitted working-set bound"
    );
}
