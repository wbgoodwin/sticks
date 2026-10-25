//! Where two members meet and a metal connector could go: a rafter on a wall
//! plate, a joist hung on a ledger, a post on a pier. The framing rules
//! record each one; the hardware crate picks the part for it.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::lumber::Nominal;

/// What kind of joint it is. Each kind takes one family of connector, and
/// the sizes of the members pick the model within it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JointKind {
    /// A rafter's birdsmouth sitting on a wall's cap plate.
    RafterOnPlate,
    /// A rafter's birdsmouth sitting on a beam, such as the porch beam.
    RafterOnBeam,
    /// A rafter's plumb cut butting the face of a ledger.
    RafterOnLedger,
    /// A pair of rafters meeting at a gable's ridge board.
    RafterAtRidge,
    /// A joist's end butting the face of a ledger or rim.
    JoistOnLedger,
    /// A joist crossing over a beam.
    JoistOnBeam,
    /// A beam sitting on top of a post.
    BeamOnPost,
    /// The foot of a post on a concrete pier.
    PostOnConcrete,
    /// The foot of a post standing on deck framing.
    PostOnDeck,
    /// A beam sitting straight on a concrete pier.
    BeamOnConcrete,
    /// A ledger fastened to the building at one stud or joist.
    LedgerToFrame,
    /// The deck tied back to the building against sideways load.
    DeckLateral,
    /// A stud tied to the plate at one of its ends.
    StudToPlate,
}

impl JointKind {
    pub const ALL: [JointKind; 13] = [
        JointKind::RafterOnPlate,
        JointKind::RafterOnBeam,
        JointKind::RafterOnLedger,
        JointKind::RafterAtRidge,
        JointKind::JoistOnLedger,
        JointKind::JoistOnBeam,
        JointKind::BeamOnPost,
        JointKind::PostOnConcrete,
        JointKind::PostOnDeck,
        JointKind::BeamOnConcrete,
        JointKind::LedgerToFrame,
        JointKind::DeckLateral,
        JointKind::StudToPlate,
    ];

    /// Plain-English name for tables, e.g. `rafter on plate`.
    pub fn name(self) -> &'static str {
        match self {
            JointKind::RafterOnPlate => "rafter on plate",
            JointKind::RafterOnBeam => "rafter on beam",
            JointKind::RafterOnLedger => "rafter on ledger",
            JointKind::RafterAtRidge => "rafters at ridge",
            JointKind::JoistOnLedger => "joist on ledger",
            JointKind::JoistOnBeam => "joist on beam",
            JointKind::BeamOnPost => "beam on post",
            JointKind::PostOnConcrete => "post on concrete",
            JointKind::PostOnDeck => "post on deck",
            JointKind::BeamOnConcrete => "beam on concrete",
            JointKind::LedgerToFrame => "ledger to frame",
            JointKind::DeckLateral => "deck lateral tie",
            JointKind::StudToPlate => "stud to plate",
        }
    }

    /// Whether the shopping list carries it without being asked. The rest
    /// are extra ties for high wind, or where a builder usually toenails.
    pub fn required(self) -> bool {
        !matches!(
            self,
            JointKind::RafterAtRidge
                | JointKind::JoistOnBeam
                | JointKind::DeckLateral
                | JointKind::StudToPlate
        )
    }
}

impl fmt::Display for JointKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// One spot in the frame that takes a connector.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Joint {
    pub kind: JointKind,
    /// The member held: the rafter, joist, beam or post.
    pub size: Nominal,
    /// Boards laminated into it, such as 2 for a doubled 2x8 beam.
    pub plies: u8,
    /// What it sits on or hangs from, when that picks the part, such as the
    /// post under a beam.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<Nominal>,
    /// Outdoors or against treated lumber, so it wants a heavier zinc coat.
    pub exposed: bool,
    /// Labels of the members it joins, such as `roof-R01`.
    pub members: Vec<String>,
}

impl Joint {
    pub fn new(kind: JointKind, size: Nominal, exposed: bool, members: &[&str]) -> Joint {
        Joint {
            kind,
            size,
            plies: 1,
            on: None,
            exposed,
            members: members.iter().map(|m| m.to_string()).collect(),
        }
    }

    pub fn plies(mut self, plies: u8) -> Joint {
        self.plies = plies;
        self
    }

    pub fn on(mut self, on: Nominal) -> Joint {
        self.on = Some(on);
        self
    }
}
