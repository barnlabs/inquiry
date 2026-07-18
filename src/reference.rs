//! Deterministic, offline reference tables for narrow factual lookups.
//!
//! The catalog is intentionally narrow. It proves a typed search/filter/export
//! contract without pretending to replace the linked primary references. No
//! network access, tolerance data, or reproduced standards prose is involved.

use serde::Serialize;
use std::{error::Error, fmt};

pub const REFERENCE_SCHEMA_VERSION: &str = "inquiry.reference/v1";
pub const MAX_QUERY_CHARS: usize = 256;
pub const MAX_SEARCH_RESULTS: usize = 256;

pub const COVERAGE_NOTE: &str = "Offline identity records for all 118 currently named elements plus selected single-start coarse-pitch ISO metric thread combinations. Element group is deliberately unassigned for lanthanides and actinides rather than resolving the disputed group-3 placement locally. The thread catalog is not a substitute for an applicable engineering standard.";
pub const ELEMENT_SCOPE_NOTE: &str = "Stable identity, period, source-backed broad category, and unambiguous group placement only; atomic-weight, isotope, oxidation-state, and material-property data are outside this catalog.";
pub const THREAD_SCOPE_NOTE: &str = "Curated common-size, single-start, coarse-pitch ISO metric combination with nominal diameter, pitch, and calculated lead only.";
pub const THREAD_LIMITATIONS: &str = "Do not use this row for tolerance class, allowance, pitch/minor diameter, gauging, tap-drill selection, fit, strength, coating allowance, or manufacturing acceptance. Verify the applicable licensed standard and drawing.";

const ELEMENT_SOURCE_IDS: &[&str] = &[
    "iupac-periodic-table-2022",
    "nist-sp-966e2019",
    "pubchem-periodic-table",
];
const THREAD_SOURCE_IDS: &[&str] = &["iso-261-1998", "iso-262-2023", "iso-724-2023"];

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceKind {
    Element,
    MetricThread,
}

impl ReferenceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Element => "element",
            Self::MetricThread => "metric_thread",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct ReferenceSource {
    pub id: &'static str,
    pub publisher: &'static str,
    pub publication: &'static str,
    pub edition_or_date: &'static str,
    pub url: &'static str,
    pub reviewed_on: &'static str,
    pub note: &'static str,
}

pub static REFERENCE_SOURCES: &[ReferenceSource] = &[
    ReferenceSource {
        id: "nist-sp-966e2019",
        publisher: "National Institute of Standards and Technology",
        publication: "Periodic Table of the Elements (NIST SP 966e2019)",
        edition_or_date: "published 2022-09-30; source page updated 2026-04-08",
        url: "https://www.nist.gov/publications/periodic-table-elements",
        reviewed_on: "2026-07-17",
        note: "Authoritative identity and periodic-placement cross-check for the static element catalog.",
    },
    ReferenceSource {
        id: "iupac-periodic-table-2022",
        publisher: "International Union of Pure and Applied Chemistry",
        publication: "Periodic Table of the Elements",
        edition_or_date: "latest table release dated 2022-05-04",
        url: "https://iupac.org/what-we-do/periodic-table-of-elements/",
        reviewed_on: "2026-07-17",
        note: "Primary authority for the 118 accepted element identities and periods; its group-3 discussion is why f-block group is left unassigned here.",
    },
    ReferenceSource {
        id: "pubchem-periodic-table",
        publisher: "National Center for Biotechnology Information",
        publication: "PubChem Periodic Table structured dataset",
        edition_or_date: "live dataset reviewed 2026-07-17",
        url: "https://pubchem.ncbi.nlm.nih.gov/rest/pug/periodictable/JSON",
        reviewed_on: "2026-07-17",
        note: "Structured cross-check for atomic number, symbol, English name, and the broad GroupBlock category labels represented locally.",
    },
    ReferenceSource {
        id: "iso-261-1998",
        publisher: "International Organization for Standardization",
        publication: "ISO 261:1998",
        edition_or_date: "1998 edition",
        url: "https://www.iso.org/standard/4165.html",
        reviewed_on: "2026-07-17",
        note: "Scope anchor for the ISO general-purpose metric thread plan; no standard prose or dimension table is copied here.",
    },
    ReferenceSource {
        id: "iso-262-2023",
        publisher: "International Organization for Standardization",
        publication: "ISO 262:2023",
        edition_or_date: "2023 edition",
        url: "https://www.iso.org/standard/85105.html",
        reviewed_on: "2026-07-17",
        note: "Scope anchor for selected commercial fastener sizes; the local table is deliberately only a common-size subset.",
    },
    ReferenceSource {
        id: "iso-724-2023",
        publisher: "International Organization for Standardization",
        publication: "ISO 724:2023",
        edition_or_date: "2023 edition",
        url: "https://www.iso.org/standard/85104.html",
        reviewed_on: "2026-07-17",
        note: "Authoritative pointer for basic dimensions; this module omits the standard's dimension tables and tolerance data.",
    },
];

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ElementCategory {
    Actinide,
    AlkaliMetal,
    AlkalineEarthMetal,
    Halogen,
    Lanthanide,
    Metalloid,
    NobleGas,
    Nonmetal,
    PostTransitionMetal,
    TransitionMetal,
}

impl ElementCategory {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Actinide => "actinide",
            Self::AlkaliMetal => "alkali_metal",
            Self::AlkalineEarthMetal => "alkaline_earth_metal",
            Self::Halogen => "halogen",
            Self::Lanthanide => "lanthanide",
            Self::Metalloid => "metalloid",
            Self::NobleGas => "noble_gas",
            Self::Nonmetal => "nonmetal",
            Self::PostTransitionMetal => "post_transition_metal",
            Self::TransitionMetal => "transition_metal",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct ElementReference {
    pub id: &'static str,
    pub atomic_number: u8,
    pub symbol: &'static str,
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub group: Option<u8>,
    pub period: u8,
    pub category: Option<ElementCategory>,
    pub source_ids: &'static [&'static str],
    pub scope_note: &'static str,
}

macro_rules! element {
    (
        $slug:literal,
        $atomic_number:literal,
        $symbol:literal,
        $name:literal,
        $aliases:expr,
        $group:expr,
        $period:literal,
        $category:ident
    ) => {
        ElementReference {
            id: concat!("element-", $slug),
            atomic_number: $atomic_number,
            symbol: $symbol,
            name: $name,
            aliases: $aliases,
            group: $group,
            period: $period,
            category: Some(ElementCategory::$category),
            source_ids: ELEMENT_SOURCE_IDS,
            scope_note: ELEMENT_SCOPE_NOTE,
        }
    };
}

pub static ELEMENTS: &[ElementReference] = &[
    element!("hydrogen", 1, "H", "Hydrogen", &[], Some(1), 1, Nonmetal),
    element!("helium", 2, "He", "Helium", &[], Some(18), 1, NobleGas),
    element!("lithium", 3, "Li", "Lithium", &[], Some(1), 2, AlkaliMetal),
    element!(
        "beryllium",
        4,
        "Be",
        "Beryllium",
        &[],
        Some(2),
        2,
        AlkalineEarthMetal
    ),
    element!("boron", 5, "B", "Boron", &[], Some(13), 2, Metalloid),
    element!("carbon", 6, "C", "Carbon", &[], Some(14), 2, Nonmetal),
    element!("nitrogen", 7, "N", "Nitrogen", &[], Some(15), 2, Nonmetal),
    element!("oxygen", 8, "O", "Oxygen", &[], Some(16), 2, Nonmetal),
    element!("fluorine", 9, "F", "Fluorine", &[], Some(17), 2, Halogen),
    element!("neon", 10, "Ne", "Neon", &[], Some(18), 2, NobleGas),
    element!("sodium", 11, "Na", "Sodium", &[], Some(1), 3, AlkaliMetal),
    element!(
        "magnesium",
        12,
        "Mg",
        "Magnesium",
        &[],
        Some(2),
        3,
        AlkalineEarthMetal
    ),
    element!(
        "aluminum",
        13,
        "Al",
        "Aluminum",
        &["Aluminium"],
        Some(13),
        3,
        PostTransitionMetal
    ),
    element!("silicon", 14, "Si", "Silicon", &[], Some(14), 3, Metalloid),
    element!(
        "phosphorus",
        15,
        "P",
        "Phosphorus",
        &[],
        Some(15),
        3,
        Nonmetal
    ),
    element!(
        "sulfur",
        16,
        "S",
        "Sulfur",
        &["Sulphur"],
        Some(16),
        3,
        Nonmetal
    ),
    element!("chlorine", 17, "Cl", "Chlorine", &[], Some(17), 3, Halogen),
    element!("argon", 18, "Ar", "Argon", &[], Some(18), 3, NobleGas),
    element!(
        "potassium",
        19,
        "K",
        "Potassium",
        &[],
        Some(1),
        4,
        AlkaliMetal
    ),
    element!(
        "calcium",
        20,
        "Ca",
        "Calcium",
        &[],
        Some(2),
        4,
        AlkalineEarthMetal
    ),
    element!(
        "scandium",
        21,
        "Sc",
        "Scandium",
        &[],
        Some(3),
        4,
        TransitionMetal
    ),
    element!(
        "titanium",
        22,
        "Ti",
        "Titanium",
        &[],
        Some(4),
        4,
        TransitionMetal
    ),
    element!(
        "vanadium",
        23,
        "V",
        "Vanadium",
        &[],
        Some(5),
        4,
        TransitionMetal
    ),
    element!(
        "chromium",
        24,
        "Cr",
        "Chromium",
        &[],
        Some(6),
        4,
        TransitionMetal
    ),
    element!(
        "manganese",
        25,
        "Mn",
        "Manganese",
        &[],
        Some(7),
        4,
        TransitionMetal
    ),
    element!("iron", 26, "Fe", "Iron", &[], Some(8), 4, TransitionMetal),
    element!(
        "cobalt",
        27,
        "Co",
        "Cobalt",
        &[],
        Some(9),
        4,
        TransitionMetal
    ),
    element!(
        "nickel",
        28,
        "Ni",
        "Nickel",
        &[],
        Some(10),
        4,
        TransitionMetal
    ),
    element!(
        "copper",
        29,
        "Cu",
        "Copper",
        &[],
        Some(11),
        4,
        TransitionMetal
    ),
    element!("zinc", 30, "Zn", "Zinc", &[], Some(12), 4, TransitionMetal),
    element!(
        "gallium",
        31,
        "Ga",
        "Gallium",
        &[],
        Some(13),
        4,
        PostTransitionMetal
    ),
    element!(
        "germanium",
        32,
        "Ge",
        "Germanium",
        &[],
        Some(14),
        4,
        Metalloid
    ),
    element!("arsenic", 33, "As", "Arsenic", &[], Some(15), 4, Metalloid),
    element!("selenium", 34, "Se", "Selenium", &[], Some(16), 4, Nonmetal),
    element!("bromine", 35, "Br", "Bromine", &[], Some(17), 4, Halogen),
    element!("krypton", 36, "Kr", "Krypton", &[], Some(18), 4, NobleGas),
    element!(
        "rubidium",
        37,
        "Rb",
        "Rubidium",
        &[],
        Some(1),
        5,
        AlkaliMetal
    ),
    element!(
        "strontium",
        38,
        "Sr",
        "Strontium",
        &[],
        Some(2),
        5,
        AlkalineEarthMetal
    ),
    element!(
        "yttrium",
        39,
        "Y",
        "Yttrium",
        &[],
        Some(3),
        5,
        TransitionMetal
    ),
    element!(
        "zirconium",
        40,
        "Zr",
        "Zirconium",
        &[],
        Some(4),
        5,
        TransitionMetal
    ),
    element!(
        "niobium",
        41,
        "Nb",
        "Niobium",
        &[],
        Some(5),
        5,
        TransitionMetal
    ),
    element!(
        "molybdenum",
        42,
        "Mo",
        "Molybdenum",
        &[],
        Some(6),
        5,
        TransitionMetal
    ),
    element!(
        "technetium",
        43,
        "Tc",
        "Technetium",
        &[],
        Some(7),
        5,
        TransitionMetal
    ),
    element!(
        "ruthenium",
        44,
        "Ru",
        "Ruthenium",
        &[],
        Some(8),
        5,
        TransitionMetal
    ),
    element!(
        "rhodium",
        45,
        "Rh",
        "Rhodium",
        &[],
        Some(9),
        5,
        TransitionMetal
    ),
    element!(
        "palladium",
        46,
        "Pd",
        "Palladium",
        &[],
        Some(10),
        5,
        TransitionMetal
    ),
    element!(
        "silver",
        47,
        "Ag",
        "Silver",
        &[],
        Some(11),
        5,
        TransitionMetal
    ),
    element!(
        "cadmium",
        48,
        "Cd",
        "Cadmium",
        &[],
        Some(12),
        5,
        TransitionMetal
    ),
    element!(
        "indium",
        49,
        "In",
        "Indium",
        &[],
        Some(13),
        5,
        PostTransitionMetal
    ),
    element!(
        "tin",
        50,
        "Sn",
        "Tin",
        &[],
        Some(14),
        5,
        PostTransitionMetal
    ),
    element!(
        "antimony",
        51,
        "Sb",
        "Antimony",
        &[],
        Some(15),
        5,
        Metalloid
    ),
    element!(
        "tellurium",
        52,
        "Te",
        "Tellurium",
        &[],
        Some(16),
        5,
        Metalloid
    ),
    element!("iodine", 53, "I", "Iodine", &[], Some(17), 5, Halogen),
    element!("xenon", 54, "Xe", "Xenon", &[], Some(18), 5, NobleGas),
    element!(
        "cesium",
        55,
        "Cs",
        "Cesium",
        &["Caesium"],
        Some(1),
        6,
        AlkaliMetal
    ),
    element!(
        "barium",
        56,
        "Ba",
        "Barium",
        &[],
        Some(2),
        6,
        AlkalineEarthMetal
    ),
    element!("lanthanum", 57, "La", "Lanthanum", &[], None, 6, Lanthanide),
    element!("cerium", 58, "Ce", "Cerium", &[], None, 6, Lanthanide),
    element!(
        "praseodymium",
        59,
        "Pr",
        "Praseodymium",
        &[],
        None,
        6,
        Lanthanide
    ),
    element!("neodymium", 60, "Nd", "Neodymium", &[], None, 6, Lanthanide),
    element!(
        "promethium",
        61,
        "Pm",
        "Promethium",
        &[],
        None,
        6,
        Lanthanide
    ),
    element!("samarium", 62, "Sm", "Samarium", &[], None, 6, Lanthanide),
    element!("europium", 63, "Eu", "Europium", &[], None, 6, Lanthanide),
    element!(
        "gadolinium",
        64,
        "Gd",
        "Gadolinium",
        &[],
        None,
        6,
        Lanthanide
    ),
    element!("terbium", 65, "Tb", "Terbium", &[], None, 6, Lanthanide),
    element!(
        "dysprosium",
        66,
        "Dy",
        "Dysprosium",
        &[],
        None,
        6,
        Lanthanide
    ),
    element!("holmium", 67, "Ho", "Holmium", &[], None, 6, Lanthanide),
    element!("erbium", 68, "Er", "Erbium", &[], None, 6, Lanthanide),
    element!("thulium", 69, "Tm", "Thulium", &[], None, 6, Lanthanide),
    element!("ytterbium", 70, "Yb", "Ytterbium", &[], None, 6, Lanthanide),
    element!("lutetium", 71, "Lu", "Lutetium", &[], None, 6, Lanthanide),
    element!(
        "hafnium",
        72,
        "Hf",
        "Hafnium",
        &[],
        Some(4),
        6,
        TransitionMetal
    ),
    element!(
        "tantalum",
        73,
        "Ta",
        "Tantalum",
        &[],
        Some(5),
        6,
        TransitionMetal
    ),
    element!(
        "tungsten",
        74,
        "W",
        "Tungsten",
        &[],
        Some(6),
        6,
        TransitionMetal
    ),
    element!(
        "rhenium",
        75,
        "Re",
        "Rhenium",
        &[],
        Some(7),
        6,
        TransitionMetal
    ),
    element!(
        "osmium",
        76,
        "Os",
        "Osmium",
        &[],
        Some(8),
        6,
        TransitionMetal
    ),
    element!(
        "iridium",
        77,
        "Ir",
        "Iridium",
        &[],
        Some(9),
        6,
        TransitionMetal
    ),
    element!(
        "platinum",
        78,
        "Pt",
        "Platinum",
        &[],
        Some(10),
        6,
        TransitionMetal
    ),
    element!("gold", 79, "Au", "Gold", &[], Some(11), 6, TransitionMetal),
    element!(
        "mercury",
        80,
        "Hg",
        "Mercury",
        &[],
        Some(12),
        6,
        TransitionMetal
    ),
    element!(
        "thallium",
        81,
        "Tl",
        "Thallium",
        &[],
        Some(13),
        6,
        PostTransitionMetal
    ),
    element!(
        "lead",
        82,
        "Pb",
        "Lead",
        &[],
        Some(14),
        6,
        PostTransitionMetal
    ),
    element!(
        "bismuth",
        83,
        "Bi",
        "Bismuth",
        &[],
        Some(15),
        6,
        PostTransitionMetal
    ),
    element!(
        "polonium",
        84,
        "Po",
        "Polonium",
        &[],
        Some(16),
        6,
        Metalloid
    ),
    element!("astatine", 85, "At", "Astatine", &[], Some(17), 6, Halogen),
    element!("radon", 86, "Rn", "Radon", &[], Some(18), 6, NobleGas),
    element!(
        "francium",
        87,
        "Fr",
        "Francium",
        &[],
        Some(1),
        7,
        AlkaliMetal
    ),
    element!(
        "radium",
        88,
        "Ra",
        "Radium",
        &[],
        Some(2),
        7,
        AlkalineEarthMetal
    ),
    element!("actinium", 89, "Ac", "Actinium", &[], None, 7, Actinide),
    element!("thorium", 90, "Th", "Thorium", &[], None, 7, Actinide),
    element!(
        "protactinium",
        91,
        "Pa",
        "Protactinium",
        &[],
        None,
        7,
        Actinide
    ),
    element!("uranium", 92, "U", "Uranium", &[], None, 7, Actinide),
    element!("neptunium", 93, "Np", "Neptunium", &[], None, 7, Actinide),
    element!("plutonium", 94, "Pu", "Plutonium", &[], None, 7, Actinide),
    element!("americium", 95, "Am", "Americium", &[], None, 7, Actinide),
    element!("curium", 96, "Cm", "Curium", &[], None, 7, Actinide),
    element!("berkelium", 97, "Bk", "Berkelium", &[], None, 7, Actinide),
    element!(
        "californium",
        98,
        "Cf",
        "Californium",
        &[],
        None,
        7,
        Actinide
    ),
    element!(
        "einsteinium",
        99,
        "Es",
        "Einsteinium",
        &[],
        None,
        7,
        Actinide
    ),
    element!("fermium", 100, "Fm", "Fermium", &[], None, 7, Actinide),
    element!(
        "mendelevium",
        101,
        "Md",
        "Mendelevium",
        &[],
        None,
        7,
        Actinide
    ),
    element!("nobelium", 102, "No", "Nobelium", &[], None, 7, Actinide),
    element!(
        "lawrencium",
        103,
        "Lr",
        "Lawrencium",
        &[],
        None,
        7,
        Actinide
    ),
    element!(
        "rutherfordium",
        104,
        "Rf",
        "Rutherfordium",
        &[],
        Some(4),
        7,
        TransitionMetal
    ),
    element!(
        "dubnium",
        105,
        "Db",
        "Dubnium",
        &[],
        Some(5),
        7,
        TransitionMetal
    ),
    element!(
        "seaborgium",
        106,
        "Sg",
        "Seaborgium",
        &[],
        Some(6),
        7,
        TransitionMetal
    ),
    element!(
        "bohrium",
        107,
        "Bh",
        "Bohrium",
        &[],
        Some(7),
        7,
        TransitionMetal
    ),
    element!(
        "hassium",
        108,
        "Hs",
        "Hassium",
        &[],
        Some(8),
        7,
        TransitionMetal
    ),
    element!(
        "meitnerium",
        109,
        "Mt",
        "Meitnerium",
        &[],
        Some(9),
        7,
        TransitionMetal
    ),
    element!(
        "darmstadtium",
        110,
        "Ds",
        "Darmstadtium",
        &[],
        Some(10),
        7,
        TransitionMetal
    ),
    element!(
        "roentgenium",
        111,
        "Rg",
        "Roentgenium",
        &[],
        Some(11),
        7,
        TransitionMetal
    ),
    element!(
        "copernicium",
        112,
        "Cn",
        "Copernicium",
        &[],
        Some(12),
        7,
        TransitionMetal
    ),
    element!(
        "nihonium",
        113,
        "Nh",
        "Nihonium",
        &[],
        Some(13),
        7,
        PostTransitionMetal
    ),
    element!(
        "flerovium",
        114,
        "Fl",
        "Flerovium",
        &[],
        Some(14),
        7,
        PostTransitionMetal
    ),
    element!(
        "moscovium",
        115,
        "Mc",
        "Moscovium",
        &[],
        Some(15),
        7,
        PostTransitionMetal
    ),
    element!(
        "livermorium",
        116,
        "Lv",
        "Livermorium",
        &[],
        Some(16),
        7,
        PostTransitionMetal
    ),
    element!(
        "tennessine",
        117,
        "Ts",
        "Tennessine",
        &[],
        Some(17),
        7,
        Halogen
    ),
    element!(
        "oganesson",
        118,
        "Og",
        "Oganesson",
        &[],
        Some(18),
        7,
        NobleGas
    ),
];

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetricThreadSeries {
    CommonCoarsePitch,
}

impl MetricThreadSeries {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CommonCoarsePitch => "common_coarse_pitch",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct MetricThreadReference {
    pub id: &'static str,
    pub designation: &'static str,
    pub nominal_diameter_mm: f64,
    pub pitch_mm: f64,
    pub starts: u8,
    pub lead_mm: f64,
    pub series: MetricThreadSeries,
    pub source_ids: &'static [&'static str],
    pub scope_note: &'static str,
    pub limitations: &'static str,
}

macro_rules! common_coarse_thread {
    ($id:literal, $designation:literal, $diameter:literal, $pitch:literal) => {
        MetricThreadReference {
            id: $id,
            designation: $designation,
            nominal_diameter_mm: $diameter,
            pitch_mm: $pitch,
            starts: 1,
            lead_mm: $pitch,
            series: MetricThreadSeries::CommonCoarsePitch,
            source_ids: THREAD_SOURCE_IDS,
            scope_note: THREAD_SCOPE_NOTE,
            limitations: THREAD_LIMITATIONS,
        }
    };
}

pub static METRIC_THREADS: &[MetricThreadReference] = &[
    common_coarse_thread!("metric-thread-m2x0.4", "M2 × 0.4", 2.0, 0.4),
    common_coarse_thread!("metric-thread-m2.5x0.45", "M2.5 × 0.45", 2.5, 0.45),
    common_coarse_thread!("metric-thread-m3x0.5", "M3 × 0.5", 3.0, 0.5),
    common_coarse_thread!("metric-thread-m4x0.7", "M4 × 0.7", 4.0, 0.7),
    common_coarse_thread!("metric-thread-m5x0.8", "M5 × 0.8", 5.0, 0.8),
    common_coarse_thread!("metric-thread-m6x1", "M6 × 1", 6.0, 1.0),
    common_coarse_thread!("metric-thread-m8x1.25", "M8 × 1.25", 8.0, 1.25),
    common_coarse_thread!("metric-thread-m10x1.5", "M10 × 1.5", 10.0, 1.5),
    common_coarse_thread!("metric-thread-m12x1.75", "M12 × 1.75", 12.0, 1.75),
    common_coarse_thread!("metric-thread-m16x2", "M16 × 2", 16.0, 2.0),
    common_coarse_thread!("metric-thread-m20x2.5", "M20 × 2.5", 20.0, 2.5),
    common_coarse_thread!("metric-thread-m24x3", "M24 × 3", 24.0, 3.0),
];

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(tag = "kind", content = "record", rename_all = "snake_case")]
pub enum ReferenceRecord<'a> {
    Element(&'a ElementReference),
    MetricThread(&'a MetricThreadReference),
}

impl ReferenceRecord<'_> {
    pub const fn kind(self) -> ReferenceKind {
        match self {
            Self::Element(_) => ReferenceKind::Element,
            Self::MetricThread(_) => ReferenceKind::MetricThread,
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Element(record) => record.id,
            Self::MetricThread(record) => record.id,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Element(record) => record.name,
            Self::MetricThread(record) => record.designation,
        }
    }

    pub const fn source_ids(self) -> &'static [&'static str] {
        match self {
            Self::Element(record) => record.source_ids,
            Self::MetricThread(record) => record.source_ids,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct ReferenceCatalog<'a> {
    pub schema_version: &'static str,
    pub network_used: bool,
    pub coverage_note: &'static str,
    pub sources: &'a [ReferenceSource],
    pub elements: &'a [ElementReference],
    pub metric_threads: &'a [MetricThreadReference],
}

pub const fn catalog() -> ReferenceCatalog<'static> {
    ReferenceCatalog {
        schema_version: REFERENCE_SCHEMA_VERSION,
        network_used: false,
        coverage_note: COVERAGE_NOTE,
        sources: REFERENCE_SOURCES,
        elements: ELEMENTS,
        metric_threads: METRIC_THREADS,
    }
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct ReferenceFilter {
    pub query: Option<String>,
    pub kind: Option<ReferenceKind>,
    pub element_category: Option<ElementCategory>,
    pub element_group: Option<u8>,
    pub element_period: Option<u8>,
    pub min_nominal_diameter_mm: Option<f64>,
    pub max_nominal_diameter_mm: Option<f64>,
    pub pitch_mm: Option<f64>,
    pub limit: Option<usize>,
}

impl ReferenceFilter {
    fn has_element_filters(&self) -> bool {
        self.element_category.is_some()
            || self.element_group.is_some()
            || self.element_period.is_some()
    }

    fn has_thread_filters(&self) -> bool {
        self.min_nominal_diameter_mm.is_some()
            || self.max_nominal_diameter_mm.is_some()
            || self.pitch_mm.is_some()
    }

    pub fn validate(&self) -> Result<(), ReferenceError> {
        if let Some(query) = &self.query {
            let length = query.chars().count();
            if length > MAX_QUERY_CHARS {
                return Err(ReferenceError::QueryTooLong {
                    actual: length,
                    maximum: MAX_QUERY_CHARS,
                });
            }
        }

        if let Some(group) = self.element_group
            && !(1..=18).contains(&group)
        {
            return Err(ReferenceError::InvalidElementGroup(group));
        }

        if let Some(period) = self.element_period
            && !(1..=7).contains(&period)
        {
            return Err(ReferenceError::InvalidElementPeriod(period));
        }

        validate_positive_number("min_nominal_diameter_mm", self.min_nominal_diameter_mm)?;
        validate_positive_number("max_nominal_diameter_mm", self.max_nominal_diameter_mm)?;
        validate_positive_number("pitch_mm", self.pitch_mm)?;

        if let (Some(minimum), Some(maximum)) =
            (self.min_nominal_diameter_mm, self.max_nominal_diameter_mm)
            && minimum > maximum
        {
            return Err(ReferenceError::ReversedDiameterRange);
        }

        if self.has_element_filters() && self.has_thread_filters() {
            return Err(ReferenceError::IncompatibleFilterFamilies);
        }

        match self.kind {
            Some(ReferenceKind::Element) if self.has_thread_filters() => {
                return Err(ReferenceError::FilterKindMismatch {
                    kind: ReferenceKind::Element,
                });
            }
            Some(ReferenceKind::MetricThread) if self.has_element_filters() => {
                return Err(ReferenceError::FilterKindMismatch {
                    kind: ReferenceKind::MetricThread,
                });
            }
            _ => {}
        }

        if let Some(limit) = self.limit {
            if limit == 0 {
                return Err(ReferenceError::ZeroLimit);
            }
            if limit > MAX_SEARCH_RESULTS {
                return Err(ReferenceError::LimitTooLarge {
                    actual: limit,
                    maximum: MAX_SEARCH_RESULTS,
                });
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceError {
    QueryTooLong { actual: usize, maximum: usize },
    InvalidElementGroup(u8),
    InvalidElementPeriod(u8),
    InvalidPositiveNumber(&'static str),
    ReversedDiameterRange,
    IncompatibleFilterFamilies,
    FilterKindMismatch { kind: ReferenceKind },
    ZeroLimit,
    LimitTooLarge { actual: usize, maximum: usize },
}

impl fmt::Display for ReferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QueryTooLong { actual, maximum } => {
                write!(
                    formatter,
                    "query has {actual} characters; maximum is {maximum}"
                )
            }
            Self::InvalidElementGroup(group) => {
                write!(
                    formatter,
                    "element group must be between 1 and 18, got {group}"
                )
            }
            Self::InvalidElementPeriod(period) => {
                write!(
                    formatter,
                    "element period must be between 1 and 7, got {period}"
                )
            }
            Self::InvalidPositiveNumber(field) => {
                write!(formatter, "{field} must be finite and greater than zero")
            }
            Self::ReversedDiameterRange => {
                write!(formatter, "minimum nominal diameter exceeds maximum")
            }
            Self::IncompatibleFilterFamilies => {
                write!(
                    formatter,
                    "element and metric-thread filters cannot be combined"
                )
            }
            Self::FilterKindMismatch { kind } => {
                write!(
                    formatter,
                    "filters do not apply to {} records",
                    kind.as_str()
                )
            }
            Self::ZeroLimit => write!(formatter, "result limit must be greater than zero"),
            Self::LimitTooLarge { actual, maximum } => {
                write!(formatter, "result limit {actual} exceeds maximum {maximum}")
            }
        }
    }
}

impl Error for ReferenceError {}

fn validate_positive_number(field: &'static str, value: Option<f64>) -> Result<(), ReferenceError> {
    if value.is_some_and(|number| !number.is_finite() || number <= 0.0) {
        return Err(ReferenceError::InvalidPositiveNumber(field));
    }
    Ok(())
}

/// Searches static records in a stable order: atomic number first, then nominal
/// thread diameter. Query terms use exact normalized tokens, which prevents a
/// lookup such as `m2` from accidentally matching `M20`.
pub fn search(filter: &ReferenceFilter) -> Result<Vec<ReferenceRecord<'static>>, ReferenceError> {
    filter.validate()?;

    let query = filter.query.as_deref().unwrap_or_default();
    let limit = filter.limit.unwrap_or(MAX_SEARCH_RESULTS);
    let mut records = Vec::new();

    let include_elements =
        !matches!(filter.kind, Some(ReferenceKind::MetricThread)) && !filter.has_thread_filters();
    if include_elements {
        for element in ELEMENTS {
            if filter
                .element_category
                .is_some_and(|category| element.category != Some(category))
                || filter
                    .element_group
                    .is_some_and(|group| element.group != Some(group))
                || filter
                    .element_period
                    .is_some_and(|period| element.period != period)
                || !element_matches_query(element, query)
            {
                continue;
            }
            records.push(ReferenceRecord::Element(element));
            if records.len() == limit {
                return Ok(records);
            }
        }
    }

    let include_threads =
        !matches!(filter.kind, Some(ReferenceKind::Element)) && !filter.has_element_filters();
    if include_threads {
        for thread in METRIC_THREADS {
            if filter.min_nominal_diameter_mm.is_some_and(|minimum| {
                thread.nominal_diameter_mm < minimum
                    && !approximately_equal(thread.nominal_diameter_mm, minimum)
            }) || filter.max_nominal_diameter_mm.is_some_and(|maximum| {
                thread.nominal_diameter_mm > maximum
                    && !approximately_equal(thread.nominal_diameter_mm, maximum)
            }) || filter
                .pitch_mm
                .is_some_and(|pitch| !approximately_equal(thread.pitch_mm, pitch))
                || !thread_matches_query(thread, query)
            {
                continue;
            }
            records.push(ReferenceRecord::MetricThread(thread));
            if records.len() == limit {
                return Ok(records);
            }
        }
    }

    Ok(records)
}

fn approximately_equal(left: f64, right: f64) -> bool {
    (left - right).abs() <= f64::EPSILON * left.abs().max(right.abs()).max(1.0) * 8.0
}

fn element_matches_query(element: &ElementReference, query: &str) -> bool {
    let mut searchable = vec![
        element.id.to_owned(),
        element.atomic_number.to_string(),
        element.symbol.to_owned(),
        element.name.to_owned(),
        "element".to_owned(),
        "period".to_owned(),
        element.period.to_string(),
    ];
    if let Some(group) = element.group {
        searchable.push("group".to_owned());
        searchable.push(group.to_string());
    }
    if let Some(category) = element.category {
        searchable.push(category.as_str().replace('_', " "));
    }
    searchable.extend(element.aliases.iter().map(|alias| (*alias).to_owned()));
    query_matches(query, searchable.iter().map(String::as_str))
}

fn thread_matches_query(thread: &MetricThreadReference, query: &str) -> bool {
    let diameter = format_decimal(thread.nominal_diameter_mm);
    let pitch = format_decimal(thread.pitch_mm);
    let compact_designation = format!("m{diameter}x{pitch}");
    let searchable = [
        thread.id,
        thread.designation,
        compact_designation.as_str(),
        "metric thread",
        "single start",
        "coarse pitch",
        thread.series.as_str(),
        diameter.as_str(),
        pitch.as_str(),
    ];
    query_matches(query, searchable)
}

fn query_matches<'a>(query: &str, searchable: impl IntoIterator<Item = &'a str>) -> bool {
    let query_tokens = normalized_tokens(query);
    if query_tokens.is_empty() {
        return true;
    }

    let searchable_tokens = searchable
        .into_iter()
        .flat_map(normalized_tokens)
        .collect::<Vec<_>>();
    query_tokens
        .iter()
        .all(|query_token| searchable_tokens.iter().any(|token| token == query_token))
}

fn normalized_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for character in value.chars() {
        if character.is_alphanumeric() || character == '.' {
            current.extend(character.to_lowercase());
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn format_decimal(value: f64) -> String {
    let mut formatted = format!("{value:.6}");
    while formatted.contains('.') && formatted.ends_with('0') {
        formatted.pop();
    }
    if formatted.ends_with('.') {
        formatted.pop();
    }
    formatted
}

pub const CSV_HEADERS: &[&str] = &[
    "schema_version",
    "kind",
    "id",
    "label",
    "atomic_number",
    "symbol",
    "group",
    "period",
    "category",
    "nominal_diameter_mm",
    "pitch_mm",
    "starts",
    "lead_mm",
    "series",
    "source_ids",
    "scope_note",
    "limitations",
];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReferenceCsvRow {
    pub schema_version: String,
    pub kind: String,
    pub id: String,
    pub label: String,
    pub atomic_number: String,
    pub symbol: String,
    pub group: String,
    pub period: String,
    pub category: String,
    pub nominal_diameter_mm: String,
    pub pitch_mm: String,
    pub starts: String,
    pub lead_mm: String,
    pub series: String,
    pub source_ids: String,
    pub scope_note: String,
    pub limitations: String,
}

impl ReferenceCsvRow {
    fn cells(&self) -> [&str; 17] {
        [
            &self.schema_version,
            &self.kind,
            &self.id,
            &self.label,
            &self.atomic_number,
            &self.symbol,
            &self.group,
            &self.period,
            &self.category,
            &self.nominal_diameter_mm,
            &self.pitch_mm,
            &self.starts,
            &self.lead_mm,
            &self.series,
            &self.source_ids,
            &self.scope_note,
            &self.limitations,
        ]
    }
}

pub fn csv_rows(records: &[ReferenceRecord<'_>]) -> Vec<ReferenceCsvRow> {
    records
        .iter()
        .map(|record| match record {
            ReferenceRecord::Element(element) => ReferenceCsvRow {
                schema_version: REFERENCE_SCHEMA_VERSION.to_owned(),
                kind: ReferenceKind::Element.as_str().to_owned(),
                id: element.id.to_owned(),
                label: element.name.to_owned(),
                atomic_number: element.atomic_number.to_string(),
                symbol: element.symbol.to_owned(),
                group: element
                    .group
                    .map(|group| group.to_string())
                    .unwrap_or_default(),
                period: element.period.to_string(),
                category: element
                    .category
                    .map(|category| category.as_str().to_owned())
                    .unwrap_or_default(),
                nominal_diameter_mm: String::new(),
                pitch_mm: String::new(),
                starts: String::new(),
                lead_mm: String::new(),
                series: String::new(),
                source_ids: element.source_ids.join(";"),
                scope_note: element.scope_note.to_owned(),
                limitations: String::new(),
            },
            ReferenceRecord::MetricThread(thread) => ReferenceCsvRow {
                schema_version: REFERENCE_SCHEMA_VERSION.to_owned(),
                kind: ReferenceKind::MetricThread.as_str().to_owned(),
                id: thread.id.to_owned(),
                label: thread.designation.to_owned(),
                atomic_number: String::new(),
                symbol: String::new(),
                group: String::new(),
                period: String::new(),
                category: String::new(),
                nominal_diameter_mm: format_decimal(thread.nominal_diameter_mm),
                pitch_mm: format_decimal(thread.pitch_mm),
                starts: thread.starts.to_string(),
                lead_mm: format_decimal(thread.lead_mm),
                series: thread.series.as_str().to_owned(),
                source_ids: thread.source_ids.join(";"),
                scope_note: thread.scope_note.to_owned(),
                limitations: thread.limitations.to_owned(),
            },
        })
        .collect()
}

/// Exports RFC-style quoted CSV with CRLF row endings and formula-trigger
/// neutralization for spreadsheet consumers.
pub fn export_csv(records: &[ReferenceRecord<'_>]) -> String {
    let mut output = CSV_HEADERS
        .iter()
        .map(|header| csv_cell(header))
        .collect::<Vec<_>>()
        .join(",");
    output.push_str("\r\n");

    for row in csv_rows(records) {
        output.push_str(
            &row.cells()
                .iter()
                .map(|cell| csv_cell(cell))
                .collect::<Vec<_>>()
                .join(","),
        );
        output.push_str("\r\n");
    }

    output
}

/// Prefixes a spreadsheet text marker when the first visible character can
/// start a formula. Leading whitespace and common Unicode formatting controls
/// are ignored while detecting the trigger.
pub fn neutralize_spreadsheet_formula(value: &str) -> String {
    let first_visible = value
        .chars()
        .find(|character| !is_ignorable_formula_prefix(*character));
    if matches!(first_visible, Some('=' | '+' | '-' | '@')) {
        format!("'{value}")
    } else {
        value.to_owned()
    }
}

pub fn csv_cell(value: &str) -> String {
    let safe = neutralize_spreadsheet_formula(value);
    format!("\"{}\"", safe.replace('"', "\"\""))
}

fn is_ignorable_formula_prefix(character: char) -> bool {
    character.is_whitespace()
        || matches!(
            character,
            '\u{feff}'
                | '\u{200b}'..='\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2060}'..='\u{206f}'
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn expected_period(atomic_number: u8) -> u8 {
        match atomic_number {
            1..=2 => 1,
            3..=10 => 2,
            11..=18 => 3,
            19..=36 => 4,
            37..=54 => 5,
            55..=86 => 6,
            87..=118 => 7,
            _ => panic!("atomic number outside catalog scope: {atomic_number}"),
        }
    }

    fn expected_unambiguous_group(atomic_number: u8) -> Option<u8> {
        match atomic_number {
            1 => Some(1),
            2 => Some(18),
            3..=4 => Some(atomic_number - 2),
            5..=10 => Some(atomic_number + 8),
            11..=12 => Some(atomic_number - 10),
            13..=18 => Some(atomic_number),
            19..=36 => Some(atomic_number - 18),
            37..=54 => Some(atomic_number - 36),
            55..=56 => Some(atomic_number - 54),
            57..=71 | 89..=103 => None,
            72..=86 => Some(atomic_number - 68),
            87..=88 => Some(atomic_number - 86),
            104..=118 => Some(atomic_number - 100),
            _ => panic!("atomic number outside catalog scope: {atomic_number}"),
        }
    }

    #[test]
    fn catalog_is_static_scoped_and_source_complete() {
        let catalog = catalog();
        assert_eq!(catalog.schema_version, REFERENCE_SCHEMA_VERSION);
        assert!(!catalog.network_used);
        assert!(catalog.coverage_note.contains("all 118"));
        assert_eq!(catalog.elements.len(), 118);
        assert!(!catalog.metric_threads.is_empty());

        let mut source_ids = BTreeSet::new();
        for source in catalog.sources {
            assert!(
                source_ids.insert(source.id),
                "duplicate source id: {}",
                source.id
            );
            assert!(source.url.starts_with("https://"));
            assert!(!source.note.is_empty());
            assert_eq!(source.reviewed_on, "2026-07-17");
        }

        for source_id in catalog
            .elements
            .iter()
            .flat_map(|element| element.source_ids)
            .chain(
                catalog
                    .metric_threads
                    .iter()
                    .flat_map(|thread| thread.source_ids),
            )
        {
            assert!(
                source_ids.contains(source_id),
                "unknown source id: {source_id}"
            );
        }
    }

    #[test]
    fn element_catalog_has_exactly_118_unique_identities_and_valid_placement() {
        let mut ids = BTreeSet::new();
        let mut symbols = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut atomic_numbers = BTreeSet::new();

        assert_eq!(ELEMENTS.len(), 118);
        for element in ELEMENTS {
            assert!(ids.insert(element.id));
            assert!(symbols.insert(element.symbol));
            assert!(names.insert(element.name));
            assert!(atomic_numbers.insert(element.atomic_number));
            assert_eq!(element.period, expected_period(element.atomic_number));
            assert_eq!(
                element.group,
                expected_unambiguous_group(element.atomic_number)
            );
            assert!(element.category.is_some());
            assert_eq!(element.source_ids, ELEMENT_SOURCE_IDS);

            if matches!(
                element.category,
                Some(ElementCategory::Lanthanide | ElementCategory::Actinide)
            ) {
                assert_eq!(element.group, None, "f-block group must remain unassigned");
            } else {
                assert!(
                    element.group.is_some_and(|group| (1..=18).contains(&group)),
                    "missing or invalid group for {}",
                    element.name
                );
            }
        }

        assert_eq!(ids.len(), 118);
        assert_eq!(symbols.len(), 118);
        assert_eq!(names.len(), 118);
        assert_eq!(
            atomic_numbers,
            (1_u8..=118).collect::<BTreeSet<_>>(),
            "atomic numbers must cover every value from 1 through 118"
        );
        assert!(
            ELEMENTS
                .windows(2)
                .all(|pair| pair[0].atomic_number < pair[1].atomic_number)
        );
        assert_eq!(ELEMENTS[0].name, "Hydrogen");
        assert_eq!(ELEMENTS[0].symbol, "H");
        assert_eq!(ELEMENTS[117].name, "Oganesson");
        assert_eq!(ELEMENTS[117].symbol, "Og");
        assert_eq!(ELEMENTS[117].group, Some(18));
    }

    #[test]
    fn thread_slice_is_sorted_single_start_and_dimensionally_consistent() {
        let mut ids = BTreeSet::new();
        for thread in METRIC_THREADS {
            assert!(ids.insert(thread.id));
            assert!(thread.nominal_diameter_mm.is_finite());
            assert!(thread.nominal_diameter_mm > 0.0);
            assert!(thread.pitch_mm.is_finite());
            assert!(thread.pitch_mm > 0.0);
            assert_eq!(thread.starts, 1);
            assert!(approximately_equal(
                thread.lead_mm,
                thread.pitch_mm * f64::from(thread.starts)
            ));
            assert_eq!(thread.source_ids, THREAD_SOURCE_IDS);
            assert_eq!(thread.scope_note, THREAD_SCOPE_NOTE);
            assert_eq!(thread.limitations, THREAD_LIMITATIONS);
            assert_eq!(
                thread.designation,
                format!(
                    "M{} × {}",
                    format_decimal(thread.nominal_diameter_mm),
                    format_decimal(thread.pitch_mm)
                )
            );
        }

        assert!(METRIC_THREADS.windows(2).all(|pair| {
            pair[0]
                .nominal_diameter_mm
                .total_cmp(&pair[1].nominal_diameter_mm)
                .is_lt()
        }));
    }

    #[test]
    fn search_is_deterministic_across_name_symbol_alias_and_number() {
        for query in ["iron 26", "FE", "transition metal fe"] {
            let results = search(&ReferenceFilter {
                query: Some(query.to_owned()),
                ..ReferenceFilter::default()
            })
            .unwrap();
            assert_eq!(results.len(), 1, "query: {query}");
            assert_eq!(results[0].id(), "element-iron");
        }

        let alias_results = search(&ReferenceFilter {
            query: Some("aluminium".to_owned()),
            ..ReferenceFilter::default()
        })
        .unwrap();
        assert_eq!(alias_results.len(), 1);
        assert_eq!(alias_results[0].id(), "element-aluminum");

        let all = search(&ReferenceFilter::default()).unwrap();
        let expected = ELEMENTS.len() + METRIC_THREADS.len();
        assert_eq!(all.len(), expected);
        assert!(
            all[..ELEMENTS.len()]
                .iter()
                .all(|record| record.kind() == ReferenceKind::Element)
        );
        assert!(
            all[ELEMENTS.len()..]
                .iter()
                .all(|record| record.kind() == ReferenceKind::MetricThread)
        );
    }

    #[test]
    fn thread_search_respects_designations_boundaries_and_numeric_filters() {
        for query in ["M6 × 1", "m6x1", "metric coarse m6"] {
            let results = search(&ReferenceFilter {
                query: Some(query.to_owned()),
                kind: Some(ReferenceKind::MetricThread),
                ..ReferenceFilter::default()
            })
            .unwrap();
            assert_eq!(results.len(), 1, "query: {query}");
            assert_eq!(results[0].id(), "metric-thread-m6x1");
        }

        let boundary_results = search(&ReferenceFilter {
            query: Some("m2".to_owned()),
            kind: Some(ReferenceKind::MetricThread),
            ..ReferenceFilter::default()
        })
        .unwrap();
        assert_eq!(boundary_results.len(), 1);
        assert_eq!(boundary_results[0].id(), "metric-thread-m2x0.4");

        let range_results = search(&ReferenceFilter {
            kind: Some(ReferenceKind::MetricThread),
            min_nominal_diameter_mm: Some(5.0),
            max_nominal_diameter_mm: Some(10.0),
            pitch_mm: Some(1.5),
            ..ReferenceFilter::default()
        })
        .unwrap();
        assert_eq!(range_results.len(), 1);
        assert_eq!(range_results[0].id(), "metric-thread-m10x1.5");
    }

    #[test]
    fn element_filters_are_scoped_and_stable() {
        let results = search(&ReferenceFilter {
            element_category: Some(ElementCategory::Nonmetal),
            element_period: Some(2),
            ..ReferenceFilter::default()
        })
        .unwrap();
        let ids = results.iter().map(|record| record.id()).collect::<Vec<_>>();
        assert_eq!(
            ids,
            vec!["element-carbon", "element-nitrogen", "element-oxygen"]
        );

        let limited = search(&ReferenceFilter {
            kind: Some(ReferenceKind::Element),
            limit: Some(2),
            ..ReferenceFilter::default()
        })
        .unwrap();
        assert_eq!(
            limited.iter().map(|record| record.id()).collect::<Vec<_>>(),
            vec!["element-hydrogen", "element-helium"]
        );
    }

    #[test]
    fn invalid_filters_fail_closed() {
        let cases = [
            (
                ReferenceFilter {
                    element_group: Some(0),
                    ..ReferenceFilter::default()
                },
                ReferenceError::InvalidElementGroup(0),
            ),
            (
                ReferenceFilter {
                    element_period: Some(8),
                    ..ReferenceFilter::default()
                },
                ReferenceError::InvalidElementPeriod(8),
            ),
            (
                ReferenceFilter {
                    pitch_mm: Some(f64::NAN),
                    ..ReferenceFilter::default()
                },
                ReferenceError::InvalidPositiveNumber("pitch_mm"),
            ),
            (
                ReferenceFilter {
                    min_nominal_diameter_mm: Some(10.0),
                    max_nominal_diameter_mm: Some(5.0),
                    ..ReferenceFilter::default()
                },
                ReferenceError::ReversedDiameterRange,
            ),
            (
                ReferenceFilter {
                    element_period: Some(2),
                    pitch_mm: Some(1.0),
                    ..ReferenceFilter::default()
                },
                ReferenceError::IncompatibleFilterFamilies,
            ),
            (
                ReferenceFilter {
                    kind: Some(ReferenceKind::Element),
                    pitch_mm: Some(1.0),
                    ..ReferenceFilter::default()
                },
                ReferenceError::FilterKindMismatch {
                    kind: ReferenceKind::Element,
                },
            ),
            (
                ReferenceFilter {
                    limit: Some(0),
                    ..ReferenceFilter::default()
                },
                ReferenceError::ZeroLimit,
            ),
        ];

        for (filter, expected) in cases {
            assert_eq!(search(&filter).unwrap_err(), expected);
        }
    }

    #[test]
    fn csv_export_round_trips_fixed_schema_and_order() {
        let records = search(&ReferenceFilter::default()).unwrap();
        let output = export_csv(&records);
        assert!(output.ends_with("\r\n"));

        let mut reader = csv::ReaderBuilder::new().from_reader(output.as_bytes());
        assert_eq!(
            reader.headers().unwrap().iter().collect::<Vec<_>>(),
            CSV_HEADERS
        );
        let parsed = reader
            .records()
            .collect::<Result<Vec<_>, csv::Error>>()
            .unwrap();
        assert_eq!(parsed.len(), records.len());
        assert_eq!(&parsed[0][1], "element");
        assert_eq!(&parsed[0][2], "element-hydrogen");
        assert_eq!(&parsed[ELEMENTS.len()][1], "metric_thread");
        assert_eq!(&parsed[ELEMENTS.len()][2], "metric-thread-m2x0.4");
        assert_eq!(&parsed[ELEMENTS.len()][9], "2");
        assert_eq!(&parsed[ELEMENTS.len()][10], "0.4");

        let lanthanum = &parsed[56];
        assert_eq!(&lanthanum[2], "element-lanthanum");
        assert_eq!(&lanthanum[6], "");
        assert_eq!(&lanthanum[8], "lanthanide");
    }

    #[test]
    fn csv_cells_quote_and_neutralize_formula_prefixes() {
        for dangerous in [
            "=1+1",
            " +1",
            "\u{feff}@command",
            "\u{202e}-2",
            "\u{200b}=hidden",
        ] {
            assert!(
                neutralize_spreadsheet_formula(dangerous).starts_with('\''),
                "payload was not neutralized: {dangerous:?}"
            );
        }
        assert_eq!(neutralize_spreadsheet_formula("M6 × 1"), "M6 × 1");

        let encoded = ["=formula", "quoted \"value\"", "comma,value", "line\nbreak"]
            .map(csv_cell)
            .join(",");
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(encoded.as_bytes());
        let record = reader.records().next().unwrap().unwrap();
        assert_eq!(&record[0], "'=formula");
        assert_eq!(&record[1], "quoted \"value\"");
        assert_eq!(&record[2], "comma,value");
        assert_eq!(&record[3], "line\nbreak");
    }
}
