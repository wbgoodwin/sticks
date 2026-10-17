//! Framing generators for floors, walls, openings and roofs.
//!
//! Walls are in place; floors and roofs come in milestone M3.

pub mod wall;

pub use wall::{frame_wall, FramedWall, FramingError, LayoutMark, Placed, Rect};
