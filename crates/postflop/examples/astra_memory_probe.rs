//! Allocation check for step 6 construction, independent of the row formulas.
//! Run without arguments for a sparse flop, with `dense-turn` for a dense turn,
//! or `dense-flop` to verify refusal before building an over-budget dense flop.
use cards::{Card, CardSet, Range};
use postflop::{
    Precision, SolveError,
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
    let case = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "sparse-flop".into());
    assert!(matches!(
        case.as_str(),
        "sparse-flop" | "dense-turn" | "dense-flop"
    ));
    let start_street = if case == "dense-turn" {
        Street::Turn
    } else {
        Street::Flop
    };
    let tree = PostflopTree::new(PostflopTreeConfig {
        starting_pot: 55,
        effective_stack: 975,
        min_bet: 10,
        start_street,
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
    let board_text = if start_street == Street::Turn {
        "9c 5d 2h Ks"
    } else {
        "9c 5d 2h"
    };
    let board: Vec<Card> = board_text
        .split_ascii_whitespace()
        .map(|s| s.parse().unwrap())
        .collect();
    let ranges = if case == "sparse-flop" {
        [Range::parse("AcAd").unwrap(), Range::parse("KcKd").unwrap()]
    } else {
        [
            Range::from_weights([1.0; 1326]).unwrap(),
            Range::from_weights([1.0; 1326]).unwrap(),
        ]
    };
    let dead = CardSet::new(&board).unwrap();
    let live = ranges.each_ref().map(|range| {
        range
            .without_cards(dead)
            .weights()
            .iter()
            .filter(|weight| **weight > 0.0)
            .count()
    });
    let estimate = PostflopMemory::for_tree_over(&tree, board.len(), 1, live).unwrap();
    let bound = estimate.working_set_bound_bytes;
    let limit = bound.min(12 * 1024 * 1024 * 1024);
    println!(
        "{case}: nodes={}, bound={bound}, shared={}, construction={}",
        estimate.expanded_nodes, estimate.shared_bytes, estimate.construction_bytes
    );
    // Excludes all allocations made before construction, including the compact
    // input tree. Thus the observed increment is a lower bound on the full job.
    let before = LIVE.load(Ordering::SeqCst);
    PEAK.store(before, Ordering::SeqCst);
    let result = PostflopGame::new(
        &board,
        ranges,
        tree,
        PostflopOptions {
            memory_limit_bytes: limit,
            precision: Precision::F64,
            threads: 1,
        },
    );
    let peak_increment = PEAK.load(Ordering::SeqCst) - before;
    if bound > limit {
        assert!(
            matches!(result, Err(SolveError::MemoryLimit { required, limit: actual }) if required == bound && actual == limit)
        );
        println!("refused: limit={limit}, incremental_peak={peak_increment}");
        assert!(
            peak_increment < estimate.shared_bytes,
            "refusal allocated the expanded game"
        );
        return;
    }
    let game = result.unwrap();
    let retained_increment = LIVE.load(Ordering::SeqCst) - before;
    println!(
        "construction: incremental_peak={peak_increment}, incremental_retained={retained_increment}, reserved={}, limit={bound}",
        game.reserved_bytes()
    );
    assert!(
        peak_increment <= estimate.shared_bytes + estimate.construction_bytes,
        "construction exceeded its shared plus transient allocation bound"
    );
}
