//! Formula language and rules engine.

pub mod builder;
pub mod builtin;
pub mod character;
pub mod choices;
pub mod class_table;
pub mod standard;
pub mod engine;
pub mod levelup;
pub mod formula;
pub mod multiclass;
pub mod origin;
pub mod overlay;
pub mod play;
pub mod value;

pub use engine::{Effect, Evaluation, Model, Op, StatDef, StatResult};
pub use formula::{Env, Formula};
pub use value::{Dice, Num, Value};
