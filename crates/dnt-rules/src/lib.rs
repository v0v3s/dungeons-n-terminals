//! Formula language and rules engine.

pub mod builder;
pub mod character;
pub mod class_table;
pub mod standard;
pub mod engine;
pub mod formula;
pub mod multiclass;
pub mod overlay;
pub mod value;

pub use engine::{Effect, Evaluation, Model, Op, StatDef, StatResult};
pub use formula::{Env, Formula};
pub use value::{Dice, Num, Value};
