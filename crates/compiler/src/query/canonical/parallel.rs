//! Runs independent steps of canonical preparation on several threads.
//!
//! With the `parallel` feature, the functions use the rayon thread pool. Without it,
//! they run on the calling thread, as WASM builds require. Both paths return results
//! in input order, so prepared indexes do not depend on thread scheduling.

/// Returns `f` applied to each of `items`, in input order.
pub(super) fn map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync + Send) -> Vec<R> {
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        items.par_iter().map(f).collect()
    }
    #[cfg(not(feature = "parallel"))]
    {
        items.iter().map(f).collect()
    }
}

/// Returns `f` applied to each of `items` with a mutable state, in input order.
///
/// `init` creates the states. Each thread creates its own states and never shares
/// them, so a state may contain caches that are not thread-safe. Results must not
/// depend on which state `f` receives.
pub(super) fn map_with_state<T: Sync, S, R: Send>(
    items: &[T],
    init: impl Fn() -> S + Sync + Send,
    f: impl Fn(&mut S, &T) -> R + Sync + Send,
) -> Vec<R> {
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        items.par_iter().map_init(init, f).collect()
    }
    #[cfg(not(feature = "parallel"))]
    {
        let mut state = init();
        items.iter().map(|item| f(&mut state, item)).collect()
    }
}
