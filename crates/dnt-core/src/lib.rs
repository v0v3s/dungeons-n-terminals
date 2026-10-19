//! The application layer of dnt, shared by every frontend: sessions over a
//! character (or a campaign), the commands that change them, and plain view
//! data to draw. Nothing here knows about terminals, browsers or phones;
//! frontends turn input into calls here and draw what comes back.

pub mod commands;
pub mod entry_fields;
pub mod export;
pub mod homebrew;
pub mod options;
pub mod rolls;
pub mod session;
pub mod spellbook;

pub use commands::{Command, Outcome, Pool};
pub use options::Choice;
pub use session::{item_entity, line_name, CharacterSession};
