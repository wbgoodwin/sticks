//! Between a [`Design`] and today's [`Project`] file, so the framing, cut
//! list and drawings work on designs unchanged.

use sticks_core::{
    Building, Deck, Floor, Length, Opening, Porch, PorchDeck, Project, Roof, Rules, Wall,
};

use crate::spec::Notes;
use crate::types::*;
use crate::validate::Note;

/// A design turned into a project, with every default it took.
#[derive(Clone, Debug)]
pub struct Converted {
    pub project: Project,
    pub assumptions: Vec<Note>,
}

/// A project with nothing in it: default rules, no stock or prices.
pub fn blank_project(name: &str) -> Project {
    Project {
        name: name.to_string(),
        on_hand: Vec::new(),
        rules: Rules::default(),
        stock: Vec::new(),
        sheets: Vec::new(),
        building: None,
        floor: None,
        roof: None,
        porch: None,
        sheathing: None,
        deck: None,
        hardware: None,
        walls: Vec::new(),
    }
}

/// The project for `design`. What a design doesn't hold (stock, prices,
/// connectors, kerf, on-hand stages) comes from `base`; everything it does
/// hold replaces what `base` had. Shape and fit problems come back as notes
/// with a path; whether the result frames is [`crate::validate`]'s job.
pub fn to_project(design: &Design, base: &Project) -> Result<Converted, Vec<Note>> {
    let mut c = Converter::default();
    let mut p = base.clone();
    p.name = design.name.clone();
    p.building = None;
    p.floor = None;
    p.roof = None;
    p.porch = None;
    p.sheathing = None;
    p.deck = None;
    p.walls.clear();

    let framing = design.framing.clone().unwrap_or(FramingSpec {
        on_center: None,
        wall_lumber: None,
        layout_from: None,
    });
    p.rules = framing.to_core(base.rules.clone(), "framing", &mut c.notes);

    c.check_ids(&design.structures);
    let building = c.only_one(&design.structures, "building");
    c.only_one(&design.structures, "porch");
    c.only_one(&design.structures, "deck");

    let mut side_walls = Vec::new();
    let mut free_walls = Vec::new();
    for (i, s) in design.structures.iter().enumerate() {
        let path = format!("structures[{i}]");
        match s {
            Structure::Building(b) => {
                c.positive(&format!("{path}.width"), b.width);
                c.positive(&format!("{path}.depth"), b.depth);
                c.positive(&format!("{path}.height"), b.height);
                p.building = Some(Building {
                    width: b.width,
                    depth: b.depth,
                    height: b.height,
                });
                p.floor = b
                    .floor
                    .as_ref()
                    .map(|f| f.to_core(Floor::default(), &format!("{path}.floor"), &mut c.notes));
                p.roof = b
                    .roof
                    .as_ref()
                    .map(|r| r.to_core(Roof::default(), &format!("{path}.roof"), &mut c.notes));
                p.sheathing = b.sheathing.clone();
                let mut seen = Vec::new();
                for (j, w) in b.walls.iter().enumerate() {
                    let wpath = format!("{path}.walls[{j}]");
                    if seen.contains(&w.side) {
                        c.problem(
                            format!("{wpath}.side"),
                            format!(
                                "the {} wall is listed twice; put everything for it in one entry",
                                w.side.id()
                            ),
                        );
                    }
                    seen.push(w.side);
                    let length = match w.side {
                        Side::Front | Side::Back => b.width,
                        Side::Left | Side::Right => b.depth,
                    };
                    // Only the front wall is `height` tall under every roof.
                    let height = match (w.side, b.roof.as_ref().and_then(|r| r.kind)) {
                        (Side::Front, _) | (_, Some(sticks_core::RoofKind::Gable)) => {
                            Some(b.height)
                        }
                        _ => None,
                    };
                    let openings = c.openings(&w.openings, length, height, &wpath);
                    c.tees(&w.tees, length, &wpath);
                    side_walls.push(Wall {
                        id: w.side.id().to_string(),
                        length: None,
                        height: None,
                        left_end: None,
                        right_end: None,
                        tees: w.tees.clone(),
                        openings,
                    });
                }
            }
            Structure::Porch(porch) => {
                match building {
                    Some(Structure::Building(b)) if b.id == porch.against.structure => {}
                    _ => c.problem(
                        format!("{path}.against.structure"),
                        format!(
                            "{:?} isn't a building in this design; a porch goes against a building's front",
                            porch.against.structure
                        ),
                    ),
                }
                let mut core = porch.to_core(Porch::default(), &path, &mut c.notes);
                core.deck = porch.deck.as_ref().map(|d| {
                    d.to_core(PorchDeck::default(), &format!("{path}.deck"), &mut c.notes)
                });
                p.porch = Some(core);
            }
            Structure::Deck(d) => {
                c.positive(&format!("{path}.width"), d.width);
                c.positive(&format!("{path}.depth"), d.depth);
                c.positive(&format!("{path}.height"), d.height);
                let mut core = d.to_core(Deck::default(), &path, &mut c.notes);
                core.width = d.width;
                core.depth = d.depth;
                core.height = d.height;
                p.deck = Some(core);
            }
            Structure::Wall(w) => {
                if Side::from_id(&w.id).is_some() {
                    c.problem(
                        format!("{path}.id"),
                        format!(
                            "{:?} names a building wall; give a freestanding wall another id",
                            w.id
                        ),
                    );
                }
                c.positive(&format!("{path}.length"), w.length);
                c.positive(&format!("{path}.height"), w.height);
                let openings = c.openings(&w.openings, w.length, Some(w.height), &path);
                c.tees(&w.tees, w.length, &path);
                free_walls.push(Wall {
                    id: w.id.clone(),
                    length: Some(w.length),
                    height: Some(w.height),
                    left_end: w.left_end,
                    right_end: w.right_end,
                    tees: w.tees.clone(),
                    openings,
                });
            }
        }
    }
    p.walls = side_walls;
    p.walls.extend(free_walls);

    if c.problems.is_empty() {
        Ok(Converted {
            project: p,
            assumptions: c.notes.assumed,
        })
    } else {
        Err(c.problems)
    }
}

#[derive(Default)]
struct Converter {
    notes: Notes,
    problems: Vec<Note>,
}

impl Converter {
    fn problem(&mut self, path: String, message: String) {
        self.problems.push(Note { path, message });
    }

    fn positive(&mut self, path: &str, v: Length) {
        if v <= Length::ZERO {
            self.problem(path.to_string(), format!("{v} has to be more than zero"));
        }
    }

    fn check_ids(&mut self, structures: &[Structure]) {
        for (i, s) in structures.iter().enumerate() {
            let id = s.id();
            if id.trim().is_empty() {
                self.problem(
                    format!("structures[{i}].id"),
                    "give it a short name, like \"shed\"".into(),
                );
            } else if structures[..i].iter().any(|o| o.id() == id) {
                self.problem(
                    format!("structures[{i}].id"),
                    format!("{id:?} is already used by another structure; ids have to be unique"),
                );
            }
        }
    }

    /// The first structure of `kind`, noting any second one: the engine
    /// frames one of each for now.
    fn only_one<'a>(&mut self, structures: &'a [Structure], kind: &str) -> Option<&'a Structure> {
        let mut first = None;
        for (i, s) in structures.iter().enumerate() {
            if s.kind() != kind {
                continue;
            }
            if first.is_some() {
                self.problem(
                    format!("structures[{i}]"),
                    format!("Sticks frames one {kind} per design for now"),
                );
            } else {
                first = Some(s);
            }
        }
        first
    }

    fn tees(&mut self, tees: &[Length], length: Length, path: &str) {
        for (k, t) in tees.iter().enumerate() {
            if *t <= Length::ZERO || *t >= length {
                self.problem(
                    format!("{path}.tees[{k}]"),
                    format!("{t} isn't inside the {length} wall"),
                );
            }
        }
    }

    /// Openings with defaults filled and places resolved to offsets from the
    /// wall's left end. `height` is the wall's, when it's the same along its
    /// length.
    fn openings(
        &mut self,
        specs: &[OpeningSpec],
        length: Length,
        height: Option<Length>,
        path: &str,
    ) -> Vec<Opening> {
        let mut out: Vec<Opening> = Vec::new();
        for (k, o) in specs.iter().enumerate() {
            let opath = format!("{path}.openings[{k}]");
            let usual = o.kind.usual();
            let width = o.width.unwrap_or(usual.width);
            let tall = o.height.unwrap_or(usual.height);
            if o.width.is_none() || o.height.is_none() {
                self.notes.say(
                    opath.clone(),
                    format!("no size given, so a {width} x {tall} rough opening"),
                );
            }
            let sill = match o.sill_height {
                Some(s) => s,
                None => {
                    self.notes
                        .assume(format!("{opath}.sill_height"), &usual.sill_height);
                    usual.sill_height
                }
            };
            let header = match o.header {
                Some(h) => h,
                None => {
                    self.notes.assume(format!("{opath}.header"), &usual.header);
                    usual.header
                }
            };
            self.positive(&format!("{opath}.width"), width);
            self.positive(&format!("{opath}.height"), tall);
            if sill < Length::ZERO {
                self.problem(
                    format!("{opath}.sill_height"),
                    "can't be below the floor".into(),
                );
            }
            let offset = match &o.place {
                Place::Left { distance } => *distance,
                Place::Right { distance } => length - *distance - width,
                Place::Center => Length::from_sixteenths((length - width).sixteenths() / 2),
            };
            if offset < Length::ZERO || offset + width > length {
                self.problem(
                    format!("{opath}.place"),
                    format!("a {width} wide opening there runs past the end of the {length} wall"),
                );
            }
            if let Some(h) = height {
                if sill + tall > h {
                    self.problem(
                        format!("{opath}.height"),
                        format!("its top, {} up, is above the {h} wall", sill + tall),
                    );
                }
            }
            for (m, other) in out.iter().enumerate() {
                if offset < other.offset + other.width && other.offset < offset + width {
                    self.problem(
                        format!("{opath}.place"),
                        format!("it overlaps {path}.openings[{m}]"),
                    );
                }
            }
            out.push(Opening {
                kind: o.kind,
                offset,
                width,
                height: tall,
                sill_height: sill,
                header,
            });
        }
        out
    }
}

/// The design for `project`, every value spelled out. Fails for the few
/// project files a design can't hold, such as a floor with no building.
pub fn from_project(project: &Project) -> Result<Design, String> {
    let mut structures = Vec::new();
    let building_id = "building";
    match &project.building {
        Some(b) => {
            let mut walls = Vec::new();
            for w in &project.walls {
                let Some(side) = Side::from_id(&w.id) else {
                    continue;
                };
                if w.length.is_some()
                    || w.height.is_some()
                    || w.left_end.is_some()
                    || w.right_end.is_some()
                {
                    return Err(format!(
                        "the {} wall sets its own size or ends, which a building's walls take from the building",
                        w.id
                    ));
                }
                walls.push(SideWall {
                    side,
                    openings: w.openings.iter().map(opening_spec).collect(),
                    tees: w.tees.clone(),
                });
            }
            structures.push(Structure::Building(BuildingSpec {
                id: building_id.to_string(),
                width: b.width,
                depth: b.depth,
                height: b.height,
                floor: project.floor.as_ref().map(FloorSpec::from_core),
                roof: project.roof.as_ref().map(RoofSpec::from_core),
                sheathing: project.sheathing.clone(),
                walls,
            }));
            if let Some(porch) = &project.porch {
                structures.push(Structure::Porch(PorchSpec::from_core(
                    porch,
                    "porch".to_string(),
                    Against {
                        structure: building_id.to_string(),
                        side: PorchSide::Front,
                    },
                    porch.deck.as_ref().map(PorchDeckSpec::from_core),
                )));
            }
        }
        None => {
            let orphan = [
                ("floor", project.floor.is_some()),
                ("roof", project.roof.is_some()),
                ("porch", project.porch.is_some()),
                ("sheathing", project.sheathing.is_some()),
            ];
            if let Some((part, _)) = orphan.iter().find(|(_, there)| *there) {
                return Err(format!("the project has a {part} but no building for it"));
            }
        }
    }
    if let Some(d) = &project.deck {
        structures.push(Structure::Deck(DeckSpec::from_core(
            d,
            "deck".to_string(),
            d.width,
            d.depth,
            d.height,
        )));
    }
    for w in &project.walls {
        if project.building.is_some() && Side::from_id(&w.id).is_some() {
            continue;
        }
        let (Some(length), Some(height)) = (w.length, w.height) else {
            return Err(format!(
                "wall {:?} has no length or height, and there's no building to take them from",
                w.id
            ));
        };
        structures.push(Structure::Wall(WallSpec {
            id: w.id.clone(),
            length,
            height,
            left_end: w.left_end,
            right_end: w.right_end,
            tees: w.tees.clone(),
            openings: w.openings.iter().map(opening_spec).collect(),
        }));
    }
    Ok(Design {
        schema: Version::V1,
        name: project.name.clone(),
        framing: Some(FramingSpec::from_core(&project.rules)),
        structures,
        questions: Vec::new(),
    })
}

fn opening_spec(o: &Opening) -> OpeningSpec {
    OpeningSpec {
        kind: o.kind,
        place: Place::Left { distance: o.offset },
        width: Some(o.width),
        height: Some(o.height),
        sill_height: Some(o.sill_height),
        header: Some(o.header),
    }
}
