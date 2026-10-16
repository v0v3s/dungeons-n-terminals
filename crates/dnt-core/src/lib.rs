//! The application layer of dnt, shared by every frontend: sessions over a
//! character (or a campaign), the commands that change them, and plain view
//! data to draw. Nothing here knows about terminals, browsers or phones;
//! frontends turn input into calls here and draw what comes back.

pub mod export;
pub mod rolls;
pub mod session;

pub use session::{item_entity, line_name, CharacterSession};
