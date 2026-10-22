//! The bridge between the Rust core and the TypeScript app.
//!
//! Everything crosses as JSON strings. Lengths go out as whole sixteenths
//! (`*_16`) with a display string beside them, so the app draws from exact
//! numbers and never formats fractions itself.

use serde::Serialize;
use sticks_core::{Length, Member, MemberKind, Project};
use sticks_cutlist::{cut_list_with_sheets, Board, CutList};
use sticks_hardware::HardwareList;
use sticks_rules::{
    bounds, frame_project, model, wall_ids, FramedProject, FramedWall, Placed, Section, Solid,
    SolidKind,
};
use wasm_bindgen::prelude::*;

/// Frames every wall in a project given as JSON (the same shape as the TOML
/// file; lengths may be builder strings like `"92-5/8\""`). A project with a
/// building lists its front, back, left and right walls first.
#[wasm_bindgen]
pub fn frame(project_json: &str) -> Result<String, JsError> {
    frame_json(project_json).map_err(|e| JsError::new(&e))
}

/// Plans the stock boards for every wall that frames, and how to cut them.
#[wasm_bindgen]
pub fn cut_list(project_json: &str) -> Result<String, JsError> {
    cut_list_json(project_json).map_err(|e| JsError::new(&e))
}

/// Reads a TOML project file into the JSON the app edits.
#[wasm_bindgen]
pub fn project_from_toml(text: &str) -> Result<String, JsError> {
    let project = Project::from_toml(text).map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string(&project).map_err(|e| JsError::new(&e.to_string()))
}

/// Writes the app's JSON project back out as a TOML project file.
#[wasm_bindgen]
pub fn project_to_toml(project_json: &str) -> Result<String, JsError> {
    let project: Project =
        serde_json::from_str(project_json).map_err(|e| JsError::new(&e.to_string()))?;
    project.to_toml().map_err(|e| JsError::new(&e.to_string()))
}

/// Parses builder notation into sixteenths of an inch.
#[wasm_bindgen]
pub fn parse_length(text: &str) -> Result<i32, JsError> {
    let l: Length = text
        .parse()
        .map_err(|e: sticks_core::ParseLengthError| JsError::new(&e.to_string()))?;
    i32::try_from(l.sixteenths()).map_err(|_| JsError::new("that length is too large"))
}

/// Formats sixteenths as feet and inches (`10' 3-1/2"`), or inches only.
#[wasm_bindgen]
pub fn format_length(sixteenths: i32, inches_only: bool) -> String {
    let l = Length::from_sixteenths(sixteenths.into());
    if inches_only {
        format!("{l:#}")
    } else {
        l.to_string()
    }
}

/// Every member, pier and subfloor sheet placed in the building, for the 3D
/// view.
#[wasm_bindgen]
pub fn model3d(project_json: &str) -> Result<String, JsError> {
    model_json(project_json).map_err(|e| JsError::new(&e))
}

/// One printable drawing as SVG: a wall by its id, or one of the plan
/// drawings `frame` lists, such as `floor`, `roof` or `porch`.
#[wasm_bindgen]
pub fn drawing(project_json: &str, which: &str) -> Result<String, JsError> {
    drawing_svg(project_json, which).map_err(|e| JsError::new(&e))
}

/// The whole build packet as one HTML page to print.
#[wasm_bindgen]
pub fn print_packet(project_json: &str) -> Result<String, JsError> {
    print_html(project_json).map_err(|e| JsError::new(&e))
}

/// The design as JSON, in the shape a classifier reads with each request:
/// sizes in builder notation and each opening labeled like `front-1`.
#[wasm_bindgen]
pub fn describe_design(project_json: &str) -> Result<String, JsError> {
    let project: Project =
        serde_json::from_str(project_json).map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string(&sticks_intent::design(&project))
        .map_err(|e| JsError::new(&e.to_string()))
}

/// The multiple-choice questions a classifier such as Jev should answer
/// next about a plain-English request, given the answers so far (a JSON
/// map of question id to answer). Ask until it comes back empty.
#[wasm_bindgen]
pub fn edit_questions(
    project_json: &str,
    request: &str,
    answers_json: &str,
) -> Result<String, JsError> {
    edit_questions_json(project_json, request, answers_json).map_err(|e| JsError::new(&e))
}

/// What a request comes to with the answers so far (a JSON map of question
/// id to answer): each clause's changes, what was assumed, what is left to
/// ask, and the changed project once nothing is.
#[wasm_bindgen]
pub fn read_edit(project_json: &str, request: &str, answers_json: &str) -> Result<String, JsError> {
    read_edit_json(project_json, request, answers_json).map_err(|e| JsError::new(&e))
}

#[derive(Serialize)]
struct FrameView {
    walls: Vec<WallView>,
    floor: Option<PlanView>,
    roof: Option<PlanView>,
    porch: Option<PlanView>,
    sheathing: Option<SheathingView>,
    /// Printable drawings besides the walls, in order.
    drawings: Vec<DrawingView>,
}

#[derive(Serialize)]
struct DrawingView {
    /// What `drawing` takes, such as `roof` or `porch-rafter`.
    id: String,
    name: String,
    /// The part it shows, for its members and warnings: `floor`, `roof`,
    /// `porch` or `sheathing`.
    part: String,
}

#[derive(Serialize)]
struct SheathingView {
    pieces: usize,
    warnings: Vec<String>,
    error: Option<String>,
}

/// The floor or roof framing in plan, seen from above.
#[derive(Serialize)]
struct PlanView {
    width_16: i64,
    depth_16: i64,
    members: Vec<MemberView>,
    warnings: Vec<String>,
    /// Set when the part can't be framed; the other fields are then empty.
    error: Option<String>,
}

#[derive(Serialize)]
struct ModelView {
    solids: Vec<SolidView>,
    /// `[min, max]` corners in inches, or null when nothing framed.
    bounds: Option<[[f64; 3]; 2]>,
    /// Parts left out because they don't frame.
    skipped: Vec<String>,
}

#[derive(Serialize)]
struct SolidView {
    label: String,
    part: String,
    kind: SolidKind,
    size: Option<String>,
    length: Option<String>,
    cuts: Vec<String>,
    profile: Vec<[f64; 2]>,
    origin: [f64; 3],
    u: [f64; 3],
    v: [f64; 3],
    depth: f64,
}

#[derive(Serialize)]
struct OpeningView {
    name: String,
    x_16: i64,
    y_16: i64,
    w_16: i64,
    h_16: i64,
}

#[derive(Serialize)]
struct WallView {
    id: String,
    length_16: i64,
    height_16: i64,
    left_height_16: i64,
    right_height_16: i64,
    members: Vec<MemberView>,
    marks: Vec<MarkView>,
    splices: Vec<SpliceView>,
    openings: Vec<OpeningView>,
    warnings: Vec<String>,
    /// Set when the wall can't be framed; the other fields are then empty.
    error: Option<String>,
}

#[derive(Serialize)]
struct MemberView {
    label: String,
    kind: MemberKind,
    size: String,
    length_16: i64,
    length: String,
    x_16: i64,
    y_16: i64,
    w_16: i64,
    h_16: i64,
    /// Corners as `[x, y]` for members that aren't rectangles; else empty.
    outline_16: Vec<[i64; 2]>,
}

#[derive(Serialize)]
struct SpliceView {
    kind: MemberKind,
    at_16: i64,
    at: String,
}

#[derive(Serialize)]
struct MarkView {
    label: String,
    kind: MemberKind,
    at_16: i64,
    at: String,
}

#[derive(Serialize)]
struct CutListView {
    boards: Vec<BoardView>,
    bom: Vec<BomView>,
    too_long: Vec<PieceView>,
    summary: SummaryView,
    /// Parts left out because they don't frame, such as `wall front`.
    skipped: Vec<String>,
    /// Every stage of the build, and whether it's kept off the list.
    sections: Vec<SectionView>,
    /// Boards for the stages the builder already has, which the shopping
    /// list leaves out. Numbered on from `boards`.
    on_hand_boards: Vec<BoardView>,
    on_hand: OnHandView,
    /// Connectors and fasteners for the stages still to buy.
    hardware: HardwareList,
}

#[derive(Serialize)]
struct SectionView {
    key: String,
    name: String,
    on_hand: bool,
}

/// What the stages left off the shopping list would have cost.
#[derive(Serialize)]
struct OnHandView {
    boards: usize,
    sheets: usize,
    total_cents: Option<u64>,
}

#[derive(Serialize)]
struct BoardView {
    id: String,
    size: String,
    length_16: i64,
    length: String,
    price_cents: Option<u32>,
    pieces: Vec<PieceView>,
    offcut_16: i64,
    offcut: String,
}

#[derive(Serialize)]
struct PieceView {
    label: String,
    kind: MemberKind,
    size: String,
    length_16: i64,
    length: String,
}

#[derive(Serialize)]
struct BomView {
    /// A lumber size like `2x4`, or a sheet like `3/4" sheet`.
    size: String,
    length_16: i64,
    length: String,
    count: usize,
    unit_price_cents: Option<u32>,
    total_cents: Option<u64>,
}

#[derive(Serialize)]
struct SummaryView {
    boards: usize,
    sheets: usize,
    pieces: usize,
    total_cents: Option<u64>,
    waste_percent: f64,
    kerf: String,
}

pub fn cut_list_json(project_json: &str) -> Result<String, String> {
    let project: Project = serde_json::from_str(project_json).map_err(|e| e.to_string())?;
    let framed = frame_project(&project).map_err(|e| e.to_string())?;
    let skipped = framed.errors().iter().map(|e| e.part.clone()).collect();
    let (buy, have) = framed.split(&project.on_hand);
    let plan = |s: &Section| {
        cut_list_with_sheets(
            &s.members,
            &s.sheets,
            &project.stock,
            &project.sheets,
            &project.rules,
        )
        .map_err(|e| e.to_string())
    };
    let cl = plan(&buy)?;
    let mut kept = plan(&have)?;
    kept.number_after(&cl);
    let sections = framed
        .sections()
        .into_iter()
        .map(|s| SectionView {
            on_hand: project.on_hand.contains(&s.key),
            key: s.key,
            name: s.name,
        })
        .collect();
    let k = kept.summary();
    let mut view = cut_list_view(cl, skipped);
    view.sections = sections;
    view.on_hand = OnHandView {
        boards: k.boards,
        sheets: k.sheets,
        total_cents: k.total_cents,
    };
    view.on_hand_boards = kept.boards.into_iter().map(board_view).collect();
    view.hardware = sticks_hardware::for_project(
        &buy.joints,
        sticks_rules::hardware_settings(&project).as_ref(),
    );
    serde_json::to_string(&view).map_err(|e| e.to_string())
}

fn board_view(b: Board) -> BoardView {
    BoardView {
        id: b.id,
        size: b.size.to_string(),
        length_16: b.length.sixteenths(),
        length: b.length.to_string(),
        price_cents: b.price_cents,
        pieces: b.pieces.into_iter().map(piece_view).collect(),
        offcut_16: b.offcut.sixteenths(),
        offcut: format!("{:#}", b.offcut),
    }
}

fn cut_list_view(cl: CutList, skipped: Vec<String>) -> CutListView {
    let s = cl.summary();
    let mut bom: Vec<BomView> = cl
        .bom()
        .into_iter()
        .map(|l| BomView {
            size: l.size.to_string(),
            length_16: l.length.sixteenths(),
            length: l.length.to_string(),
            count: l.count,
            unit_price_cents: l.unit_price_cents,
            total_cents: l.total_cents,
        })
        .collect();
    bom.extend(cl.sheet_bom().into_iter().map(|l| BomView {
        size: format!("{:#} sheet", l.thickness),
        length_16: l.length.sixteenths(),
        length: format!("{} x {}", l.width, l.length),
        count: l.count,
        unit_price_cents: l.unit_price_cents,
        total_cents: l.total_cents,
    }));
    CutListView {
        summary: SummaryView {
            boards: s.boards,
            sheets: s.sheets,
            pieces: s.pieces,
            total_cents: s.total_cents,
            waste_percent: s.waste_percent,
            kerf: format!("{:#}", cl.kerf),
        },
        bom,
        boards: cl.boards.into_iter().map(board_view).collect(),
        too_long: cl.too_long.into_iter().map(piece_view).collect(),
        skipped,
        sections: Vec::new(),
        on_hand_boards: Vec::new(),
        on_hand: OnHandView {
            boards: 0,
            sheets: 0,
            total_cents: Some(0),
        },
        hardware: HardwareList::default(),
    }
}

fn piece_view(m: Member) -> PieceView {
    PieceView {
        length: m.length.to_tape_string(),
        label: m.label,
        kind: m.kind,
        size: m.size.to_string(),
        length_16: m.length.sixteenths(),
    }
}

#[derive(Serialize)]
struct EditView {
    steps: Vec<StepView>,
    /// The project with every change made, once nothing is left to ask.
    project: Option<Project>,
    /// Why the changed project won't frame, if it won't.
    problems: Vec<String>,
}

#[derive(Serialize)]
struct StepView {
    text: String,
    /// What in the design it changes, once that's settled, such as
    /// `width` or `front-1`.
    target: Option<String>,
    /// Each change in builder English.
    changes: Vec<String>,
    assumed: Vec<String>,
    question: Option<sticks_intent::Question>,
    problem: Option<String>,
}

pub fn edit_questions_json(
    project_json: &str,
    request: &str,
    answers_json: &str,
) -> Result<String, String> {
    let project: Project = serde_json::from_str(project_json).map_err(|e| e.to_string())?;
    let answers: sticks_intent::Answers =
        serde_json::from_str(answers_json).map_err(|e| e.to_string())?;
    serde_json::to_string(&sticks_intent::questions(&project, request, &answers))
        .map_err(|e| e.to_string())
}

pub fn read_edit_json(
    project_json: &str,
    request: &str,
    answers_json: &str,
) -> Result<String, String> {
    let project: Project = serde_json::from_str(project_json).map_err(|e| e.to_string())?;
    let answers: sticks_intent::Answers =
        serde_json::from_str(answers_json).map_err(|e| e.to_string())?;
    let reading = sticks_intent::interpret(&project, request, &answers);
    let (changed, problems) = match reading.edits() {
        Some(edits) => match sticks_intent::apply_all(&project, &edits) {
            Ok(p) => {
                let problems = sticks_intent::framing_problems(&p);
                (Some(p), problems)
            }
            Err(e) => (None, vec![e.to_string()]),
        },
        None => (None, Vec::new()),
    };
    let steps = reading
        .steps
        .into_iter()
        .map(|s| StepView {
            text: s.text,
            target: s.target,
            changes: s.edits.iter().map(|e| e.to_string()).collect(),
            assumed: s.assumed,
            question: s.question,
            problem: s.problem,
        })
        .collect();
    let view = EditView {
        steps,
        project: changed,
        problems,
    };
    serde_json::to_string(&view).map_err(|e| e.to_string())
}

fn framed(project_json: &str) -> Result<(Project, FramedProject), String> {
    let project: Project = serde_json::from_str(project_json).map_err(|e| e.to_string())?;
    let framed = frame_project(&project).map_err(|e| e.to_string())?;
    Ok((project, framed))
}

pub fn frame_json(project_json: &str) -> Result<String, String> {
    let (project, framed) = framed(project_json)?;
    let drawings = drawing_list(&framed);
    // Walls come back in the order frame_project lists them.
    let walls = framed
        .walls
        .into_iter()
        .zip(wall_ids(&project))
        .map(|(result, id)| match result {
            Ok(framed) => wall_view(framed),
            Err(e) => {
                let wall = project.walls.iter().find(|w| w.id == id);
                let length = wall.and_then(|w| w.length).unwrap_or_default();
                let height = wall.and_then(|w| w.height).unwrap_or_default();
                WallView {
                    id,
                    length_16: length.sixteenths(),
                    height_16: height.sixteenths(),
                    left_height_16: height.sixteenths(),
                    right_height_16: height.sixteenths(),
                    members: Vec::new(),
                    marks: Vec::new(),
                    splices: Vec::new(),
                    openings: Vec::new(),
                    warnings: Vec::new(),
                    error: Some(e.reason),
                }
            }
        })
        .collect();
    let floor = framed.floor.map(|r| match r {
        Ok(f) => plan_view(f.width, f.depth, &f.members, f.warnings),
        Err(e) => plan_error(e.reason),
    });
    let roof = framed.roof.map(|r| match r {
        Ok(f) => plan_view(f.width, f.depth, &f.members, f.warnings),
        Err(e) => plan_error(e.reason),
    });
    let porch = framed.porch.map(|r| match r {
        Ok(p) => {
            let mut members = p.members.clone();
            let mut warnings = p.warnings.clone();
            if let Some(d) = &p.deck {
                members.extend(d.members.iter().cloned());
                warnings.extend(d.warnings.iter().map(|w| format!("deck: {w}")));
            }
            plan_view(p.width, p.depth, &members, warnings)
        }
        Err(e) => plan_error(e.reason),
    });
    let sheathing = framed.sheathing.map(|r| match r {
        Ok(s) => SheathingView {
            pieces: s.panels().count(),
            warnings: s.warnings,
            error: None,
        },
        Err(e) => SheathingView {
            pieces: 0,
            warnings: Vec::new(),
            error: Some(e.reason),
        },
    });
    serde_json::to_string(&FrameView {
        walls,
        floor,
        roof,
        porch,
        sheathing,
        drawings,
    })
    .map_err(|e| e.to_string())
}

/// Every plan drawing the framed project can print, by id.
fn drawing_list(framed: &FramedProject) -> Vec<DrawingView> {
    let mut out = Vec::new();
    let mut add = |id: &str, name: &str, part: &str| {
        out.push(DrawingView {
            id: id.to_string(),
            name: name.to_string(),
            part: part.to_string(),
        })
    };
    if framed.floor.is_some() {
        add("floor", "Floor", "floor");
        add("subfloor", "Subfloor", "floor");
    }
    if let Some(roof) = &framed.roof {
        add("roof", "Roof", "roof");
        match roof {
            Ok(r) if r.rafters.len() > 1 => {
                add("rafter", "Front rafter", "roof");
                add("rafter-2", "Back rafter", "roof");
            }
            _ => add("rafter", "Rafter", "roof"),
        }
    }
    if let Some(porch) = &framed.porch {
        add("porch", "Porch", "porch");
        add("porch-rafter", "Porch rafter", "porch");
        if matches!(porch, Ok(p) if p.deck.is_some()) {
            add("deck", "Deck", "porch");
        }
    }
    if let Some(s) = &framed.sheathing {
        let (walls, roofs) = match s {
            Ok(s) => (!s.walls.is_empty(), !s.roofs.is_empty()),
            Err(_) => (true, false),
        };
        if walls {
            add("wall-sheathing", "Wall sheathing", "sheathing");
        }
        if roofs {
            add("roof-sheathing", "Roof sheathing", "sheathing");
        }
    }
    out
}

fn plan_view(width: Length, depth: Length, members: &[Placed], warnings: Vec<String>) -> PlanView {
    PlanView {
        width_16: width.sixteenths(),
        depth_16: depth.sixteenths(),
        members: members.iter().cloned().map(member_view).collect(),
        warnings,
        error: None,
    }
}

fn plan_error(reason: String) -> PlanView {
    PlanView {
        width_16: 0,
        depth_16: 0,
        members: Vec::new(),
        warnings: Vec::new(),
        error: Some(reason),
    }
}

fn member_view(p: Placed) -> MemberView {
    MemberView {
        length: p.member.length.to_tape_string(),
        label: p.member.label,
        kind: p.member.kind,
        size: p.member.size.to_string(),
        length_16: p.member.length.sixteenths(),
        x_16: p.rect.x.sixteenths(),
        y_16: p.rect.y.sixteenths(),
        w_16: p.rect.w.sixteenths(),
        h_16: p.rect.h.sixteenths(),
        outline_16: p
            .outline
            .iter()
            .map(|(x, y)| [x.sixteenths(), y.sixteenths()])
            .collect(),
    }
}

pub fn model_json(project_json: &str) -> Result<String, String> {
    let (project, framed) = framed(project_json)?;
    let solids = model(&project, &framed);
    let view = ModelView {
        bounds: bounds(&solids),
        skipped: framed.errors().iter().map(|e| e.part.clone()).collect(),
        solids: solids.into_iter().map(solid_view).collect(),
    };
    serde_json::to_string(&view).map_err(|e| e.to_string())
}

fn solid_view(s: Solid) -> SolidView {
    SolidView {
        size: s.member.as_ref().map(|m| m.size.to_string()),
        length: s.member.as_ref().map(|m| m.length.to_tape_string()),
        cuts: s
            .member
            .iter()
            .flat_map(|m| &m.cuts)
            .map(|c| c.describe())
            .collect(),
        label: s.label,
        part: s.part,
        kind: s.kind,
        profile: s.profile,
        origin: s.origin,
        u: s.u,
        v: s.v,
        depth: s.depth,
    }
}

pub fn drawing_svg(project_json: &str, which: &str) -> Result<String, String> {
    let (project, framed) = framed(project_json)?;
    let name = &project.name;
    let from = project.rules.layout_from;
    let unframed = |part: &str| format!("the {part} doesn't frame");
    match which {
        "floor" | "subfloor" => {
            let floor = framed
                .floor
                .ok_or("this project has no floor")?
                .map_err(|e| e.reason)?;
            Ok(if which == "floor" {
                sticks_export::floor_svg(name, &floor, from)
            } else {
                sticks_export::subfloor_svg(name, &floor)
            })
        }
        "roof" | "rafter" | "rafter-2" => {
            let roof = framed
                .roof
                .ok_or("this project has no roof")?
                .map_err(|e| e.reason)?;
            match which {
                "roof" => Ok(sticks_export::roof_svg(name, &roof, from)),
                _ => {
                    let i = usize::from(which == "rafter-2");
                    let r = roof
                        .rafters
                        .get(i)
                        .ok_or("this roof has one kind of rafter")?;
                    Ok(sticks_export::rafter_svg(name, r))
                }
            }
        }
        "porch" | "porch-rafter" | "deck" => {
            let porch = framed
                .porch
                .ok_or("this project has no porch")?
                .map_err(|e| e.reason)?;
            match which {
                "porch" => Ok(sticks_export::porch_svg(name, &porch, from)),
                "porch-rafter" => Ok(sticks_export::rafter_svg(name, &porch.rafter)),
                _ => sticks_export::deck_svg(name, &porch, from)
                    .ok_or_else(|| "this porch has no deck".to_string()),
            }
        }
        "wall-sheathing" | "roof-sheathing" => {
            let s = framed
                .sheathing
                .ok_or("this project has no sheathing")?
                .map_err(|e| e.reason)?;
            sticks_export::sheathing_svg(name, &s, which == "roof-sheathing")
                .ok_or_else(|| "nothing is sheathed there".to_string())
        }
        id => {
            let i = wall_ids(&project)
                .iter()
                .position(|w| w == id)
                .ok_or_else(|| format!("there is no wall {id}"))?;
            let wall = framed.walls[i].as_ref().map_err(|_| unframed("wall"))?;
            Ok(sticks_export::wall_svg(name, wall, from))
        }
    }
}

pub fn print_html(project_json: &str) -> Result<String, String> {
    let (project, framed) = framed(project_json)?;
    let (buy, _) = framed.split(&project.on_hand);
    let cl = cut_list_with_sheets(
        &buy.members,
        &buy.sheets,
        &project.stock,
        &project.sheets,
        &project.rules,
    )
    .map_err(|e| e.to_string())?;
    let hw = sticks_hardware::for_project(
        &buy.joints,
        sticks_rules::hardware_settings(&project).as_ref(),
    );
    Ok(sticks_export::print_html(&project, &framed, &cl, &hw))
}

fn wall_view(w: FramedWall) -> WallView {
    WallView {
        id: w.id,
        length_16: w.length.sixteenths(),
        height_16: w.height.sixteenths(),
        left_height_16: w.left_height.sixteenths(),
        right_height_16: w.right_height.sixteenths(),
        members: w.members.into_iter().map(member_view).collect(),
        splices: w
            .splices
            .into_iter()
            .map(|s| SpliceView {
                kind: s.kind,
                at_16: s.at.sixteenths(),
                at: format!("{:#}", s.at),
            })
            .collect(),
        marks: w
            .marks
            .into_iter()
            .map(|m| MarkView {
                at: format!("{:#}", m.at),
                label: m.label,
                kind: m.kind,
                at_16: m.at.sixteenths(),
            })
            .collect(),
        openings: w
            .openings
            .into_iter()
            .map(|o| OpeningView {
                name: o.name,
                x_16: o.rect.x.sixteenths(),
                y_16: o.rect.y.sixteenths(),
                w_16: o.rect.w.sixteenths(),
                h_16: o.rect.h.sixteenths(),
            })
            .collect(),
        warnings: w.warnings,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_json_with_builder_lengths() {
        let json = r#"{
            "name": "x",
            "wall": [
                {"id": "A", "length": "8'", "height": "97-1/8\""},
                {"id": "B", "length": "8'", "height": "2\""}
            ]
        }"#;
        let out: serde_json::Value = serde_json::from_str(&frame_json(json).unwrap()).unwrap();
        let a = &out["walls"][0];
        assert_eq!(a["length_16"], 96 * 16);
        assert_eq!(a["members"][3]["length"], "92-5/8\"");
        assert!(a["error"].is_null());
        assert!(out["walls"][1]["error"]
            .as_str()
            .unwrap()
            .contains("too short"));
    }

    #[test]
    fn plans_cuts_from_json() {
        let json = r#"{
            "name": "x",
            "rules": {"kerf": "1/8\""},
            "stock": [{"size": "2x4", "lengths": ["8'", "16'"], "prices_cents": [498, 1149]}],
            "wall": [
                {"id": "A", "length": "8'", "height": "97-1/8\""},
                {"id": "B", "length": "8'", "height": "2\""}
            ]
        }"#;
        let out: serde_json::Value = serde_json::from_str(&cut_list_json(json).unwrap()).unwrap();
        assert_eq!(out["skipped"][0], "wall B");
        assert!(out["too_long"].as_array().unwrap().is_empty());
        let pieces: usize = out["boards"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["pieces"].as_array().unwrap().len())
            .sum();
        assert_eq!(pieces, out["summary"]["pieces"]);
        assert!(out["summary"]["total_cents"].is_u64());
        assert_eq!(out["bom"][0]["size"], "2x4");
    }

    #[test]
    fn frames_a_whole_building() {
        let json = project_from_toml(include_str!("../../../examples/shed.toml")).unwrap();
        let out: serde_json::Value = serde_json::from_str(&frame_json(&json).unwrap()).unwrap();
        let ids: Vec<_> = out["walls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| w["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, sticks_rules::SIDES);
        let left = &out["walls"][2];
        assert!(left["error"].is_null());
        assert_ne!(left["left_height_16"], left["right_height_16"]);
        assert_eq!(
            left["members"][1]["outline_16"].as_array().unwrap().len(),
            4
        );
        assert!(!out["walls"][0]["splices"].as_array().unwrap().is_empty());

        let plan: serde_json::Value = serde_json::from_str(&cut_list_json(&json).unwrap()).unwrap();
        assert!(plan["too_long"].as_array().unwrap().is_empty());
        assert!(plan["summary"]["sheets"].as_u64().unwrap() >= 8);
        assert!(plan["bom"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["size"] == "3/4\" sheet"));
    }

    #[test]
    fn keeps_on_hand_sections_off_the_shopping_list() {
        let json = project_from_toml(include_str!("../../../examples/shed.toml")).unwrap();
        let all: serde_json::Value = serde_json::from_str(&cut_list_json(&json).unwrap()).unwrap();
        let mut project: serde_json::Value = serde_json::from_str(&json).unwrap();
        project["on_hand"] = serde_json::json!(["floor", "subfloor"]);
        let json = project.to_string();
        let plan: serde_json::Value = serde_json::from_str(&cut_list_json(&json).unwrap()).unwrap();

        let floor = plan["sections"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["key"] == "floor")
            .unwrap();
        assert_eq!(floor["name"], "Floor framing");
        assert_eq!(floor["on_hand"], true);
        let boards = |v: &serde_json::Value, key: &str| v[key].as_array().unwrap().len();
        assert!(boards(&plan, "on_hand_boards") > 0);
        assert_eq!(
            plan["on_hand"]["boards"].as_u64().unwrap() as usize,
            boards(&plan, "on_hand_boards")
        );
        assert!(boards(&plan, "boards") < boards(&all, "boards"));
        // The floor's post caps and bases come off with it.
        let caps = |v: &serde_json::Value| {
            v["hardware"]["parts"]
                .as_array()
                .unwrap()
                .iter()
                .find(|l| l["model"] == "BC4Z")
                .map_or(0, |l| l["count"].as_u64().unwrap())
        };
        assert_eq!(caps(&all), 12);
        assert_eq!(caps(&plan), 0);
        // No subfloor sheets left to buy; the shed's sheathing is a
        // different thickness.
        assert_eq!(
            plan["summary"]["sheets"].as_u64().unwrap()
                + plan["on_hand"]["sheets"].as_u64().unwrap(),
            all["summary"]["sheets"].as_u64().unwrap()
        );
        assert!(!plan["bom"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["size"] == "3/4\" sheet"));
        // Every board id is used once across the two plans.
        let mut ids: Vec<_> = plan["boards"]
            .as_array()
            .unwrap()
            .iter()
            .chain(plan["on_hand_boards"].as_array().unwrap())
            .map(|b| b["id"].as_str().unwrap().to_string())
            .collect();
        let count = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), count);
        assert!(print_html(&json)
            .unwrap()
            .contains("since you have it: Floor framing, Subfloor."));
        let toml = project_to_toml(&json).unwrap();
        let back: serde_json::Value =
            serde_json::from_str(&project_from_toml(&toml).unwrap()).unwrap();
        assert_eq!(back["on_hand"], serde_json::json!(["floor", "subfloor"]));
    }

    #[test]
    fn models_drawings_and_packet() {
        let json = project_from_toml(include_str!("../../../examples/shed.toml")).unwrap();
        let out: serde_json::Value = serde_json::from_str(&frame_json(&json).unwrap()).unwrap();
        assert!(out["floor"]["error"].is_null());
        assert!(!out["roof"]["members"].as_array().unwrap().is_empty());
        assert_eq!(out["walls"][0]["openings"][0]["name"], "Door 1");

        let m: serde_json::Value = serde_json::from_str(&model_json(&json).unwrap()).unwrap();
        let solids = m["solids"].as_array().unwrap();
        let rafter = solids.iter().find(|s| s["kind"] == "rafter").unwrap();
        assert_eq!(rafter["part"], "roof");
        assert!(rafter["cuts"][1]
            .as_str()
            .unwrap()
            .starts_with("birdsmouth"));
        assert!(solids.iter().any(|s| s["kind"] == "pier"));
        assert_eq!(m["bounds"].as_array().unwrap().len(), 2);

        let drawings: Vec<&str> = out["drawings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            drawings,
            [
                "floor",
                "subfloor",
                "roof",
                "rafter",
                "porch",
                "porch-rafter",
                "deck",
                "wall-sheathing",
                "roof-sheathing"
            ]
        );
        assert!(out["porch"]["error"].is_null());
        assert!(out["sheathing"]["pieces"].as_u64().unwrap() > 20);
        assert!(solids.iter().any(|s| s["kind"] == "sheathing"));
        assert!(solids.iter().any(|s| s["part"] == "deck"));
        for which in ["front", "left"].iter().chain(&drawings) {
            assert!(
                drawing_svg(&json, which).unwrap().starts_with("<svg"),
                "{which}"
            );
        }
        assert!(drawing_svg(&json, "rafter-2").is_err());
        assert!(drawing_svg(&json, "garage").is_err());
        assert!(print_html(&json).unwrap().contains("Shopping list"));
    }

    #[test]
    fn toml_round_trips_through_json() {
        let toml = include_str!("../../../examples/shed.toml");
        let json = project_from_toml(toml).unwrap();
        let back = project_to_toml(&json).unwrap();
        assert_eq!(
            Project::from_toml(&back).unwrap(),
            Project::from_toml(toml).unwrap()
        );
    }

    #[test]
    fn reads_a_request_into_a_changed_project() {
        let project = serde_json::to_string(
            &Project::from_toml(include_str!("../../../examples/shed.toml")).unwrap(),
        )
        .unwrap();
        // First the target, then, once it's settled, its own questions.
        let ids = |answers: &str| -> Vec<String> {
            let asks: serde_json::Value = serde_json::from_str(
                &edit_questions_json(&project, "add a window", answers).unwrap(),
            )
            .unwrap();
            asks.as_array()
                .unwrap()
                .iter()
                .map(|a| a["id"].as_str().unwrap().to_string())
                .collect()
        };
        assert_eq!(ids("{}"), ["0.target"]);
        let target = r#"{"0.target": {"pick": "new_opening", "confidence": 0.95}}"#;
        assert!(ids(target).contains(&"0.wall".to_string()));

        let sure = r#"{"0.target": {"pick": "new_opening", "confidence": 0.95},
                       "0.kind": {"pick": "window", "confidence": 0.95}}"#;
        let out: serde_json::Value =
            serde_json::from_str(&read_edit_json(&project, "add a window", sure).unwrap()).unwrap();
        assert_eq!(out["steps"][0]["question"]["id"], "0.wall");
        assert!(out["project"].is_null());

        let answered = sure.replace(
            "}}",
            r#"}, "0.wall": {"pick": "back", "confidence": 1, "by_user": true}}"#,
        );
        let out: serde_json::Value =
            serde_json::from_str(&read_edit_json(&project, "add a window", &answered).unwrap())
                .unwrap();
        assert_eq!(
            out["steps"][0]["changes"][0],
            "Add a 3' x 3' window to the back wall, 7' 6\" from its left end"
        );
        assert!(out["problems"].as_array().unwrap().is_empty());
        let back = out["project"]["wall"]
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["id"] == "back")
            .unwrap();
        assert_eq!(back["opening"].as_array().unwrap().len(), 1);
    }
}
