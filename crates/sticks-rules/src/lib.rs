//! Framing generators for floors, walls, openings and roofs.

pub mod building;
pub mod floor;
pub mod layout;
pub mod model;
pub mod roof;
pub mod wall;

pub use building::{frame_project, FramedProject, SIDES};
pub use floor::{frame_floor, Bay, FramedFloor, Pier, PlacedSheet};
pub use layout::{LayoutMark, Placed, Rect, Splice};
pub use model::{bounds, model, wall_ids, OtherKind, Solid, SolidKind};
pub use roof::{frame_roof, lean_to_heights, FramedRoof, LeanToHeights, RafterDetail};
pub use wall::{
    frame_wall, frame_wall_shaped, FramedWall, FramingError, OpeningOutline, WallShape,
    PLATE_JOINT_OFFSET,
};
