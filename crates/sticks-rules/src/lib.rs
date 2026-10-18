//! Framing generators for floors, walls, openings, roofs, porches and
//! sheathing.

pub mod building;
pub mod floor;
pub mod joints;
pub mod layout;
pub mod model;
pub mod porch;
pub mod roof;
pub mod sheathing;
pub mod wall;

pub use building::{frame_project, side_plan_y, FramedProject, Section, SIDES};
pub use floor::{frame_floor, Bay, FramedFloor, Pier, PlacedSheet};
pub use layout::{LayoutMark, Placed, Rect, Splice};
pub use model::{bounds, model, wall_ids, OtherKind, Solid, SolidKind};
pub use porch::{frame_porch, DeckHeights, FramedDeck, FramedPorch, PorchHeights};
pub use roof::{
    frame_roof, gable_studs, wall_heights, FramedRoof, RafterDetail, RafterTop, Ridge, RoofPlane,
    WallHeights,
};
pub use sheathing::{solid_pieces, FramedSheathing, SheathedFace, SheathingPanel};
pub use wall::{
    frame_wall, frame_wall_shaped, FramedWall, FramingError, OpeningOutline, WallShape,
    PLATE_JOINT_OFFSET,
};
