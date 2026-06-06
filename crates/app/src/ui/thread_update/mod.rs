//! MVU-style contracts for the thread surface.
//!
//! `ThreadView` owns GPUI handles and rendering caches, but state changes enter
//! through actions and leave through explicit effects.

mod action;
mod effect;

pub use action::ThreadAction;
pub use effect::ThreadEffect;
