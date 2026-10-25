//! The joints in each framed part that take a connector, found from the
//! members the framers placed: what kind each one is and the sizes on both
//! sides. Nothing here names a brand; the hardware crate picks the parts.

use sticks_core::{BeamStyle, Cut, Joint, JointKind, Length, MemberKind, Nominal};

use crate::deck::FramedDeck;
use crate::floor::{FramedFloor, Pier};
use crate::layout::{Placed, Rect};
use crate::porch::{FramedPorch, FramedPorchDeck};
use crate::roof::FramedRoof;
use crate::wall::FramedWall;

/// Members this close to a pier's center, across the beam, are plies of the
/// beam over it.
const NEAR: Length = Length::inches(6);

impl FramedWall {
    /// Each full-height stud tied to the plate at both ends.
    pub fn joints(&self) -> Vec<Joint> {
        let mut out = Vec::new();
        for p in &self.members {
            if matches!(p.member.kind, MemberKind::Stud | MemberKind::KingStud) {
                for _ in 0..2 {
                    out.push(Joint::new(
                        JointKind::StudToPlate,
                        p.member.size,
                        false,
                        &[&p.member.label],
                    ));
                }
            }
        }
        out
    }
}

impl FramedFloor {
    /// Each joist on every beam line it crosses, and each beam on its posts
    /// or piers. Skids on grade take nothing.
    pub fn joints(&self) -> Vec<Joint> {
        let mut out = Vec::new();
        let beams: Vec<&Placed> = self
            .members
            .iter()
            .filter(|p| matches!(p.member.kind, MemberKind::Beam | MemberKind::Skid))
            .collect();
        let lines = beam_lines(&beams);
        for j in self
            .members
            .iter()
            .filter(|p| p.member.kind == MemberKind::Joist)
        {
            for _ in 0..lines {
                out.push(Joint::new(
                    JointKind::JoistOnBeam,
                    j.member.size,
                    true,
                    &[&j.member.label],
                ));
            }
        }
        let posts: Vec<&Placed> = self
            .members
            .iter()
            .filter(|p| p.member.kind == MemberKind::Post)
            .collect();
        for (i, pier) in self.piers.iter().enumerate() {
            let Some((size, plies, label)) = beam_over(&beams, pier) else {
                continue;
            };
            match posts.get(i) {
                Some(post) => {
                    out.push(
                        Joint::new(
                            JointKind::BeamOnPost,
                            size,
                            true,
                            &[&label, &post.member.label],
                        )
                        .plies(plies)
                        .on(post.member.size),
                    );
                    out.push(Joint::new(
                        JointKind::PostOnConcrete,
                        post.member.size,
                        true,
                        &[&post.member.label],
                    ));
                }
                None => out.push(
                    Joint::new(JointKind::BeamOnConcrete, size, true, &[&label]).plies(plies),
                ),
            }
        }
        out
    }
}

impl FramedRoof {
    /// Each rafter on every plate its birdsmouths sit on, and each pair of
    /// gable rafters at the ridge.
    pub fn joints(&self) -> Vec<Joint> {
        let mut out = Vec::new();
        for r in self
            .members
            .iter()
            .filter(|p| p.member.kind == MemberKind::Rafter)
        {
            for _ in 0..birdsmouths(r) {
                out.push(Joint::new(
                    JointKind::RafterOnPlate,
                    r.member.size,
                    false,
                    &[&r.member.label],
                ));
            }
            if self.ridge.is_some() && r.member.label.ends_with('F') {
                let back = format!("{}B", &r.member.label[..r.member.label.len() - 1]);
                out.push(Joint::new(
                    JointKind::RafterAtRidge,
                    r.member.size,
                    false,
                    &[&r.member.label, &back],
                ));
            }
        }
        out
    }
}

impl FramedPorch {
    /// Each rafter hung on the ledger and sitting on the beam, the ledger
    /// fastened at each rafter, and the beam and posts at each post. The
    /// deck's joints are its own.
    pub fn joints(&self) -> Vec<Joint> {
        let mut out = Vec::new();
        let ledger = self
            .members
            .iter()
            .find(|p| p.member.kind == MemberKind::Ledger);
        for r in self
            .members
            .iter()
            .filter(|p| p.member.kind == MemberKind::Rafter)
        {
            let label = r.member.label.as_str();
            if let Some(l) = ledger {
                out.push(
                    Joint::new(
                        JointKind::RafterOnLedger,
                        r.member.size,
                        true,
                        &[label, &l.member.label],
                    )
                    .on(l.member.size),
                );
                out.push(Joint::new(
                    JointKind::LedgerToFrame,
                    l.member.size,
                    true,
                    &[&l.member.label],
                ));
            }
            for _ in 0..birdsmouths(r) {
                out.push(Joint::new(
                    JointKind::RafterOnBeam,
                    r.member.size,
                    true,
                    &[label],
                ));
            }
        }
        let beams: Vec<&Placed> = self
            .members
            .iter()
            .filter(|p| p.member.kind == MemberKind::Beam)
            .collect();
        for post in self
            .members
            .iter()
            .filter(|p| p.member.kind == MemberKind::Post)
        {
            let center = post.rect.x + post.rect.w.mul_div(1, 2);
            let over: Vec<&&Placed> = beams.iter().filter(|b| spans(&b.rect, center)).collect();
            if let Some(b) = over.first() {
                out.push(
                    Joint::new(
                        JointKind::BeamOnPost,
                        b.member.size,
                        true,
                        &[&b.member.label, &post.member.label],
                    )
                    .plies(plies(&over))
                    .on(post.member.size),
                );
            }
            let foot = if self.deck.is_some() {
                JointKind::PostOnDeck
            } else {
                JointKind::PostOnConcrete
            };
            out.push(Joint::new(
                foot,
                post.member.size,
                true,
                &[&post.member.label],
            ));
        }
        out
    }
}

impl FramedPorchDeck {
    /// Each joist hung on the ledger and crossing the beam, the ledger
    /// fastened at each joist, the beam on its piers, and two lateral ties
    /// back to the building.
    pub fn joints(&self) -> Vec<Joint> {
        let mut out = Vec::new();
        let ledger = self
            .members
            .iter()
            .find(|p| p.member.kind == MemberKind::Ledger);
        let mut size = None;
        for j in self
            .members
            .iter()
            .filter(|p| p.member.kind == MemberKind::Joist)
        {
            size = Some(j.member.size);
            let label = j.member.label.as_str();
            if let Some(l) = ledger {
                out.push(
                    Joint::new(
                        JointKind::JoistOnLedger,
                        j.member.size,
                        true,
                        &[label, &l.member.label],
                    )
                    .on(l.member.size),
                );
                out.push(Joint::new(
                    JointKind::LedgerToFrame,
                    l.member.size,
                    true,
                    &[&l.member.label],
                ));
            }
            out.push(Joint::new(
                JointKind::JoistOnBeam,
                j.member.size,
                true,
                &[label],
            ));
        }
        let beams: Vec<&Placed> = self
            .members
            .iter()
            .filter(|p| p.member.kind == MemberKind::Beam)
            .collect();
        for pier in &self.piers {
            if let Some((size, plies, label)) = beam_over(&beams, pier) {
                out.push(Joint::new(JointKind::BeamOnConcrete, size, true, &[&label]).plies(plies));
            }
        }
        if let (Some(size), Some(l)) = (size, ledger) {
            for _ in 0..2 {
                out.push(Joint::new(
                    JointKind::DeckLateral,
                    size,
                    true,
                    &[&l.member.label],
                ));
            }
        }
        out
    }
}

impl FramedDeck {
    /// Each joist on every beam it crosses, or hung at both ends between
    /// flush beams, and each beam on its posts or piers.
    pub fn joints(&self) -> Vec<Joint> {
        let mut out = Vec::new();
        let flush = self.beam_style == BeamStyle::Flush;
        let lines = self.beam_y.len();
        for j in self
            .members
            .iter()
            .filter(|p| p.member.kind == MemberKind::Joist)
        {
            if flush {
                for _ in 0..2 {
                    out.push(
                        Joint::new(
                            JointKind::JoistOnLedger,
                            j.member.size,
                            true,
                            &[&j.member.label],
                        )
                        .on(self.beam),
                    );
                }
            } else {
                for _ in 0..lines {
                    out.push(Joint::new(
                        JointKind::JoistOnBeam,
                        j.member.size,
                        true,
                        &[&j.member.label],
                    ));
                }
            }
        }
        let beams: Vec<&Placed> = self
            .members
            .iter()
            .filter(|p| p.member.kind == MemberKind::Beam)
            .collect();
        let posts: Vec<&Placed> = self
            .members
            .iter()
            .filter(|p| p.member.kind == MemberKind::Post)
            .collect();
        for (i, pier) in self.piers.iter().enumerate() {
            let label = beam_over(&beams, pier)
                .map(|(_, _, l)| l)
                .unwrap_or_default();
            match posts.get(i) {
                Some(post) => {
                    out.push(
                        Joint::new(
                            JointKind::BeamOnPost,
                            self.beam,
                            true,
                            &[&label, &post.member.label],
                        )
                        .plies(self.beam_plies)
                        .on(post.member.size),
                    );
                    out.push(Joint::new(
                        JointKind::PostOnConcrete,
                        post.member.size,
                        true,
                        &[&post.member.label],
                    ));
                }
                None => out.push(
                    Joint::new(JointKind::BeamOnConcrete, self.beam, true, &[&label])
                        .plies(self.beam_plies),
                ),
            }
        }
        out
    }
}

fn birdsmouths(p: &Placed) -> usize {
    p.member
        .cuts
        .iter()
        .filter(|c| matches!(c, Cut::Birdsmouth { .. }))
        .count()
}

fn spans(r: &Rect, x: Length) -> bool {
    r.x <= x && x <= r.right()
}

/// The beam over `pier`: its size, how many plies, and the label of one.
fn beam_over(beams: &[&Placed], pier: &Pier) -> Option<(Nominal, u8, String)> {
    let over: Vec<&&Placed> = beams
        .iter()
        .filter(|b| spans(&b.rect, pier.x))
        .filter(|b| (b.rect.y + b.rect.h.mul_div(1, 2) - pier.y).abs() < NEAR)
        .collect();
    let first = over.first()?;
    Some((first.member.size, plies(&over), first.member.label.clone()))
}

/// Plies side by side, counting the two pieces of a ply that meet over a
/// support once.
fn plies(over: &[&&Placed]) -> u8 {
    let mut ys: Vec<Length> = over.iter().map(|b| b.rect.y).collect();
    ys.sort();
    ys.dedup();
    ys.len() as u8
}

/// How many lines of beams run under the joists, counting a built-up beam
/// once.
fn beam_lines(beams: &[&Placed]) -> usize {
    let mut ys: Vec<Length> = beams.iter().map(|b| b.rect.y).collect();
    ys.sort();
    ys.dedup();
    let mut lines = 0;
    let mut last: Option<Length> = None;
    for y in ys {
        if last.is_none_or(|l| y - l >= NEAR) {
            lines += 1;
            last = Some(y);
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use sticks_core::{FloorSupport, Project};

    use crate::building::frame_project;

    use super::*;

    fn shed() -> Project {
        Project::from_toml(include_str!("../../../examples/shed.toml")).unwrap()
    }

    fn count(project: &Project) -> BTreeMap<JointKind, usize> {
        let framed = frame_project(project).unwrap();
        let mut out = BTreeMap::new();
        for s in framed.sections() {
            for j in s.joints {
                *out.entry(j.kind).or_default() += 1;
            }
        }
        out
    }

    #[test]
    fn finds_every_joint_in_the_shed() {
        let mut p = shed();
        p.floor.as_mut().unwrap().support = FloorSupport::Piers;
        let n = count(&p);
        // 15 rafters, each on the front and back walls.
        assert_eq!(n[&JointKind::RafterOnPlate], 30);
        // 15 porch rafters on the ledger and over the beam; the ledger
        // fastened at each; 15 deck joists the same.
        assert_eq!(n[&JointKind::RafterOnLedger], 15);
        assert_eq!(n[&JointKind::RafterOnBeam], 15);
        assert_eq!(n[&JointKind::JoistOnLedger], 15);
        assert_eq!(n[&JointKind::LedgerToFrame], 30);
        // Four posts under the doubled beam, standing on the deck.
        assert_eq!(n[&JointKind::BeamOnPost], 4);
        assert_eq!(n[&JointKind::PostOnDeck], 4);
        // 12 floor piers and 4 deck piers, each under a beam.
        assert_eq!(n[&JointKind::BeamOnConcrete], 16);
        // 15 floor joists over 3 beams, and 15 deck joists over 1.
        assert_eq!(n[&JointKind::JoistOnBeam], 60);
        assert_eq!(n[&JointKind::DeckLateral], 2);
        assert!(!n.contains_key(&JointKind::RafterAtRidge));
    }

    #[test]
    fn porch_beam_counts_its_plies() {
        let framed = frame_project(&shed()).unwrap();
        let porch = framed.porch.unwrap().unwrap();
        for j in porch.joints() {
            if j.kind == JointKind::BeamOnPost {
                assert_eq!(j.plies, 2, "{:?}", j.members);
                assert_eq!(j.on, Some(Nominal::new(4, 4)));
                assert_eq!(j.size, Nominal::new(2, 8));
            }
        }
    }

    #[test]
    fn posts_on_piers_carry_the_floor() {
        let n = count(&shed());
        assert_eq!(n[&JointKind::BeamOnPost], 4 + 12);
        assert_eq!(n[&JointKind::PostOnConcrete], 12);
        // Only the deck's piers still carry a beam straight.
        assert_eq!(n[&JointKind::BeamOnConcrete], 4);
    }

    #[test]
    fn gable_pairs_meet_at_the_ridge() {
        let mut p = shed();
        p.roof.as_mut().unwrap().kind = sticks_core::RoofKind::Gable;
        p.porch = None;
        let n = count(&p);
        assert_eq!(n[&JointKind::RafterAtRidge], 15);
        assert_eq!(n[&JointKind::RafterOnPlate], 30);
    }
}
