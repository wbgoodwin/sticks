//! Connectors for the joints the framing rules find, picked from a parts
//! catalog, with the nails or screws to hang them.
//!
//! The catalog that ships is Simpson Strong-Tie's: for each kind of joint,
//! the part that fits the sizes on both sides, in plain galvanized for dry
//! framing and ZMAX where the joint is outdoors or against treated lumber.
//! It picks the usual part for the sizes; it doesn't check loads.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sticks_core::{Fastening, Hardware, Joint, JointKind, Nominal};

/// Said wherever the hardware list is shown.
pub const NOTICE: &str = "Connectors are the usual part for each joint's lumber sizes; Sticks doesn't check loads. Check them against your local code and the maker's catalog.";

/// A parts catalog: connectors and the fasteners they take.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub brand: String,
    #[serde(rename = "part")]
    pub parts: Vec<Part>,
    #[serde(rename = "fastener")]
    pub fasteners: Vec<Fastener>,
}

/// One connector model and the joints it fits.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    /// Model for dry framing, such as `H2.5A`.
    pub model: String,
    /// Model for joints outdoors or against treated lumber, such as
    /// `H2.5AZ`. Left out when the one model does both.
    #[serde(default)]
    pub exposed: Option<String>,
    /// What it is, such as `hurricane tie`.
    pub name: String,
    pub joints: Vec<JointKind>,
    /// Sizes of the member held. Empty fits any.
    #[serde(default)]
    pub sizes: Vec<Nominal>,
    /// Plies of the member held. Default 1.
    #[serde(default = "one")]
    pub plies: u8,
    /// Sizes of what it sits on or hangs from, such as the post under a
    /// beam. Empty fits any.
    #[serde(default)]
    pub on: Vec<Nominal>,
    /// Pieces at each joint. Default 1.
    #[serde(default = "one_u32")]
    pub each: u32,
    /// Fasteners per piece, by the catalog's fastener key.
    #[serde(default)]
    pub nails: BTreeMap<String, u32>,
    /// Anything else it needs, such as an anchor bolt.
    #[serde(default)]
    pub note: Option<String>,
}

/// A nail, and the connector screw that can go in its holes instead.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fastener {
    /// What parts call it, such as `10d`.
    pub key: String,
    pub name: String,
    /// The connector screw used in place of this nail.
    pub screw: String,
    pub screw_name: String,
    /// Pieces in a box of the nails and of the screws.
    pub nails_per_box: u32,
    pub screws_per_box: u32,
}

fn one() -> u8 {
    1
}

fn one_u32() -> u32 {
    1
}

impl Catalog {
    /// Simpson Strong-Tie, the catalog that ships with Sticks.
    pub fn simpson() -> Catalog {
        toml::from_str(include_str!("simpson.toml")).expect("the Simpson catalog parses")
    }

    /// The part for `joint`, or none when nothing in the catalog fits.
    pub fn fit(&self, joint: &Joint) -> Option<&Part> {
        self.parts.iter().find(|p| {
            p.joints.contains(&joint.kind)
                && (p.sizes.is_empty() || p.sizes.contains(&joint.size))
                && p.plies == joint.plies
                && (p.on.is_empty() || joint.on.is_some_and(|on| p.on.contains(&on)))
        })
    }

    /// The part sold as `model`, galvanized or ZMAX.
    pub fn by_model(&self, model: &str) -> Option<&Part> {
        self.parts
            .iter()
            .find(|p| p.model == model || p.exposed.as_deref() == Some(model))
    }
}

impl Part {
    /// The model to buy for a joint outdoors or not.
    pub fn model_for(&self, exposed: bool) -> &str {
        match (&self.exposed, exposed) {
            (Some(z), true) => z,
            _ => &self.model,
        }
    }
}

/// A line on the hardware list: one connector model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PartLine {
    pub model: String,
    pub name: String,
    /// The joints it's for, such as `rafter on plate`.
    pub joints: Vec<String>,
    pub count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub unit_price_cents: Option<u32>,
    pub total_cents: Option<u64>,
}

/// A line on the hardware list: nails or screws of one kind.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FastenerLine {
    /// What its price is kept under, such as `10d` or `SD10212`.
    pub key: String,
    pub name: String,
    pub count: u32,
    pub per_box: u32,
    pub boxes: u32,
    pub box_price_cents: Option<u32>,
    pub total_cents: Option<u64>,
}

/// Everything to buy for the joints: connectors, then their fasteners.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct HardwareList {
    pub brand: String,
    pub parts: Vec<PartLine>,
    pub fasteners: Vec<FastenerLine>,
    /// Joints nothing in the catalog fits, and models it doesn't know.
    pub warnings: Vec<String>,
    /// The whole list's cost, when every line has a price.
    pub total_cents: Option<u64>,
}

/// The connectors and fasteners for `joints` from the Simpson catalog, as
/// the project's settings ask, or with the defaults when it has none.
pub fn for_project(joints: &[Joint], settings: Option<&Hardware>) -> HardwareList {
    hardware_list(
        &Catalog::simpson(),
        joints,
        &settings.cloned().unwrap_or_default(),
    )
}

/// The connectors and fasteners for `joints`, as `settings` asks.
pub fn hardware_list(catalog: &Catalog, joints: &[Joint], settings: &Hardware) -> HardwareList {
    let mut parts: BTreeMap<String, PartLine> = BTreeMap::new();
    let mut nails: BTreeMap<String, u32> = BTreeMap::new();
    let mut missing: BTreeMap<String, u32> = BTreeMap::new();
    for joint in joints.iter().filter(|j| settings.wants(j.kind)) {
        let chosen = match settings.parts.get(&joint.kind) {
            Some(model) => match catalog.by_model(model) {
                Some(p) => Some((p.model_for(joint.exposed).to_string(), p)),
                None => {
                    add_part(&mut parts, model, model, joint.kind, 1, None);
                    *missing
                        .entry(format!(
                            "{model} isn't in the {} catalog, so its nails aren't counted",
                            catalog.brand
                        ))
                        .or_default() += 1;
                    None
                }
            },
            None => match catalog.fit(joint) {
                Some(p) => Some((p.model_for(joint.exposed).to_string(), p)),
                None => {
                    *missing.entry(no_fit(catalog, joint)).or_default() += 1;
                    None
                }
            },
        };
        if let Some((model, p)) = chosen {
            add_part(
                &mut parts,
                &model,
                &p.name,
                joint.kind,
                p.each,
                p.note.clone(),
            );
            for (key, n) in &p.nails {
                *nails.entry(key.clone()).or_default() += n * p.each;
            }
        }
    }

    let price = |key: &str| settings.prices_cents.get(key).copied();
    let mut list = HardwareList {
        brand: catalog.brand.clone(),
        ..HardwareList::default()
    };
    for mut line in parts.into_values() {
        line.unit_price_cents = price(&line.model);
        line.total_cents = line
            .unit_price_cents
            .map(|c| u64::from(c) * u64::from(line.count));
        list.parts.push(line);
    }
    let mut fasteners: BTreeMap<String, FastenerLine> = BTreeMap::new();
    for (key, count) in nails {
        let Some(f) = catalog.fasteners.iter().find(|f| f.key == key) else {
            list.warnings
                .push(format!("the catalog has no fastener called {key}"));
            continue;
        };
        let (key, name, per_box) = match settings.fasten_with {
            Fastening::Nails => (f.key.clone(), f.name.clone(), f.nails_per_box),
            Fastening::Screws => (f.screw.clone(), f.screw_name.clone(), f.screws_per_box),
        };
        let line = fasteners.entry(key.clone()).or_insert(FastenerLine {
            key,
            name,
            count: 0,
            per_box: per_box.max(1),
            boxes: 0,
            box_price_cents: None,
            total_cents: None,
        });
        line.count += count;
    }
    for mut line in fasteners.into_values() {
        line.boxes = line.count.div_ceil(line.per_box);
        line.box_price_cents = price(&line.key);
        line.total_cents = line
            .box_price_cents
            .map(|c| u64::from(c) * u64::from(line.boxes));
        list.fasteners.push(line);
    }
    for (warning, n) in missing {
        list.warnings.push(if n == 1 {
            warning
        } else {
            format!("{warning} ({n} joints)")
        });
    }
    let totals: Vec<Option<u64>> = list
        .parts
        .iter()
        .map(|l| l.total_cents)
        .chain(list.fasteners.iter().map(|l| l.total_cents))
        .collect();
    if !totals.is_empty() && totals.iter().all(Option::is_some) {
        list.total_cents = Some(totals.into_iter().flatten().sum());
    }
    list
}

fn add_part(
    parts: &mut BTreeMap<String, PartLine>,
    model: &str,
    name: &str,
    kind: JointKind,
    each: u32,
    note: Option<String>,
) {
    let line = parts.entry(model.to_string()).or_insert(PartLine {
        model: model.to_string(),
        name: name.to_string(),
        joints: Vec::new(),
        count: 0,
        note,
        unit_price_cents: None,
        total_cents: None,
    });
    line.count += each;
    let joint = kind.name().to_string();
    if !line.joints.contains(&joint) {
        line.joints.push(joint);
    }
}

/// Why nothing fits, such as `no Simpson Strong-Tie part for a 3-ply 2x8
/// beam on a 4x4 post`.
fn no_fit(catalog: &Catalog, joint: &Joint) -> String {
    let plies = if joint.plies > 1 {
        format!("{}-ply ", joint.plies)
    } else {
        String::new()
    };
    let on = joint.on.map(|o| format!(" on a {o}")).unwrap_or_default();
    let hint = match joint.kind {
        JointKind::BeamOnConcrete => "; anchor it to suit the piers, or set the beams on posts",
        JointKind::PostOnDeck => "; run the posts down to piers, or pick a base to suit",
        _ => "",
    };
    format!(
        "no {} part for a {plies}{} {}{on}{hint}",
        catalog.brand,
        joint.size,
        joint.kind.name()
    )
}

#[cfg(test)]
mod tests {
    use sticks_core::{FloorSupport, Project};
    use sticks_rules::frame_project;

    use super::*;

    fn shed() -> Project {
        Project::from_toml(include_str!("../../../examples/shed.toml")).unwrap()
    }

    fn list(project: &Project) -> HardwareList {
        let framed = frame_project(project).unwrap();
        let (buy, _) = framed.split(&project.on_hand);
        let settings = project.hardware.clone().unwrap_or_default();
        hardware_list(&Catalog::simpson(), &buy.joints, &settings)
    }

    fn count(list: &HardwareList, model: &str) -> u32 {
        list.parts
            .iter()
            .find(|l| l.model == model)
            .map_or(0, |l| l.count)
    }

    #[test]
    fn the_catalog_loads() {
        let c = Catalog::simpson();
        assert!(c.parts.len() > 20);
        for p in &c.parts {
            for key in p.nails.keys() {
                assert!(c.fasteners.iter().any(|f| &f.key == key), "{key}");
            }
        }
    }

    #[test]
    fn picks_parts_for_the_shed() {
        let l = list(&shed());
        // Main rafters indoors, porch rafters outdoors.
        assert_eq!(count(&l, "H2.5A"), 30);
        assert_eq!(count(&l, "H2.5AZ"), 15);
        assert_eq!(count(&l, "LRU26Z"), 15);
        assert_eq!(count(&l, "LUS26Z"), 15);
        assert_eq!(count(&l, "BCS2-2/4Z"), 4);
        // Two screws at each of 15 porch rafters and 15 deck joists.
        assert_eq!(count(&l, "SDWS22500DB"), 60);
        // Optional ties stay off.
        assert_eq!(count(&l, "DTT2Z"), 0);
        // The floor's beams sit on posts on piers.
        assert_eq!(count(&l, "BC4Z"), 12);
        assert_eq!(count(&l, "ABU44Z"), 12);
        assert!(l.parts.iter().any(|x| x.note.is_some()));
        // The deck beam straight on piers and posts on the deck have no part.
        assert!(l.warnings.iter().any(|w| w.contains("beam on concrete")));
        assert!(l.warnings.iter().any(|w| w.contains("post on deck")));
        // 45 ties of 10 8d nails, and 16d for the rafter hangers.
        let nails = |key: &str| l.fasteners.iter().find(|f| f.key == key).unwrap().count;
        assert_eq!(nails("8d"), 450);
        assert_eq!(nails("16d"), 15 * 9 + 12 * 12 + 12 * 12);
        assert_eq!(l.total_cents, None);
    }

    #[test]
    fn beams_on_piers_need_an_anchor() {
        let mut p = shed();
        p.floor.as_mut().unwrap().support = FloorSupport::Piers;
        let l = list(&p);
        assert_eq!(count(&l, "BC4Z"), 0);
        assert!(l
            .warnings
            .iter()
            .any(|w| w.contains("4x6 beam on concrete") && w.contains("(16 joints)")));
    }

    #[test]
    fn on_hand_stages_drop_their_hardware() {
        let mut p = shed();
        p.on_hand = vec!["roof".to_string(), "deck".to_string()];
        let l = list(&p);
        assert_eq!(count(&l, "H2.5A"), 0);
        assert_eq!(count(&l, "LUS26Z"), 0);
        assert_eq!(count(&l, "LRU26Z"), 15);
    }

    #[test]
    fn settings_add_swap_and_price() {
        let mut p = shed();
        let mut hw = Hardware {
            fasten_with: Fastening::Screws,
            high_wind: true,
            ..Hardware::default()
        };
        hw.parts.insert(JointKind::RafterOnPlate, "H1".to_string());
        hw.parts
            .insert(JointKind::BeamOnConcrete, "none".to_string());
        hw.parts.insert(JointKind::PostOnDeck, "none".to_string());
        p.hardware = Some(hw);
        let l = list(&p);
        // H1 isn't in the catalog: listed, but its nails aren't counted.
        assert_eq!(count(&l, "H1"), 30);
        assert!(l.warnings.iter().any(|w| w.starts_with("H1 isn't")));
        // High wind adds the deck ties, joist ties and stud ties.
        assert_eq!(count(&l, "DTT2Z"), 2);
        assert!(count(&l, "SP4") > 0);
        assert!(l.fasteners.iter().all(|f| f.key.starts_with("SD")));
        assert_eq!(l.total_cents, None);

        // Price everything and the list has a total.
        let mut hw = p.hardware.clone().unwrap();
        for line in &l.parts {
            hw.prices_cents.insert(line.model.clone(), 100);
        }
        for line in &l.fasteners {
            hw.prices_cents.insert(line.key.clone(), 1000);
        }
        p.hardware = Some(hw);
        let l = list(&p);
        let parts: u64 = l.parts.iter().map(|x| u64::from(x.count) * 100).sum();
        let boxes: u64 = l.fasteners.iter().map(|x| u64::from(x.boxes) * 1000).sum();
        assert_eq!(l.total_cents, Some(parts + boxes));
    }
}
