// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

#[rerun::rerun_type]
#[rust(derive(PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer1 {
    #[rerun(component_required)]
    pub fuzz1001: rerun::testing::components::AffixFuzzer1,

    #[rerun(component_required)]
    pub fuzz1002: rerun::testing::components::AffixFuzzer2,

    #[rerun(component_required)]
    pub fuzz1003: rerun::testing::components::AffixFuzzer3,

    #[rerun(component_required)]
    pub fuzz1004: rerun::testing::components::AffixFuzzer4,

    #[rerun(component_required)]
    pub fuzz1005: rerun::testing::components::AffixFuzzer5,

    #[rerun(component_required)]
    pub fuzz1006: rerun::testing::components::AffixFuzzer6,

    #[rerun(component_required)]
    pub fuzz1007: rerun::testing::components::AffixFuzzer7,

    #[rerun(component_required)]
    pub fuzz1008: rerun::testing::components::AffixFuzzer8,

    #[rerun(component_required)]
    pub fuzz1009: rerun::testing::components::AffixFuzzer9,

    #[rerun(component_required)]
    pub fuzz1010: rerun::testing::components::AffixFuzzer10,

    #[rerun(component_required)]
    pub fuzz1011: rerun::testing::components::AffixFuzzer11,

    #[rerun(component_required)]
    pub fuzz1012: rerun::testing::components::AffixFuzzer12,

    #[rerun(component_required)]
    pub fuzz1013: rerun::testing::components::AffixFuzzer13,

    #[rerun(component_required)]
    pub fuzz1014: rerun::testing::components::AffixFuzzer14,

    #[rerun(component_required)]
    pub fuzz1015: rerun::testing::components::AffixFuzzer15,

    #[rerun(component_required)]
    pub fuzz1016: rerun::testing::components::AffixFuzzer16,

    #[rerun(component_required)]
    pub fuzz1017: rerun::testing::components::AffixFuzzer17,

    #[rerun(component_required)]
    pub fuzz1018: rerun::testing::components::AffixFuzzer18,

    #[rerun(component_required)]
    pub fuzz1019: rerun::testing::components::AffixFuzzer19,

    #[rerun(component_required)]
    pub fuzz1020: rerun::testing::components::AffixFuzzer20,

    #[rerun(component_required)]
    pub fuzz1021: rerun::testing::components::AffixFuzzer21,

    #[rerun(component_required)]
    pub fuzz1022: rerun::testing::components::AffixFuzzer22,
}

#[rerun::rerun_type]
#[rust(derive(PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer2 {
    #[rerun(component_required)]
    pub fuzz1101: Vec<rerun::testing::components::AffixFuzzer1>,

    #[rerun(component_required)]
    pub fuzz1102: Vec<rerun::testing::components::AffixFuzzer2>,

    #[rerun(component_required)]
    pub fuzz1103: Vec<rerun::testing::components::AffixFuzzer3>,

    #[rerun(component_required)]
    pub fuzz1104: Vec<rerun::testing::components::AffixFuzzer4>,

    #[rerun(component_required)]
    pub fuzz1105: Vec<rerun::testing::components::AffixFuzzer5>,

    #[rerun(component_required)]
    pub fuzz1106: Vec<rerun::testing::components::AffixFuzzer6>,

    #[rerun(component_required)]
    pub fuzz1107: Vec<rerun::testing::components::AffixFuzzer7>,

    #[rerun(component_required)]
    pub fuzz1108: Vec<rerun::testing::components::AffixFuzzer8>,

    #[rerun(component_required)]
    pub fuzz1109: Vec<rerun::testing::components::AffixFuzzer9>,

    #[rerun(component_required)]
    pub fuzz1110: Vec<rerun::testing::components::AffixFuzzer10>,

    #[rerun(component_required)]
    pub fuzz1111: Vec<rerun::testing::components::AffixFuzzer11>,

    #[rerun(component_required)]
    pub fuzz1112: Vec<rerun::testing::components::AffixFuzzer12>,

    #[rerun(component_required)]
    pub fuzz1113: Vec<rerun::testing::components::AffixFuzzer13>,

    #[rerun(component_required)]
    pub fuzz1114: Vec<rerun::testing::components::AffixFuzzer14>,

    #[rerun(component_required)]
    pub fuzz1115: Vec<rerun::testing::components::AffixFuzzer15>,

    #[rerun(component_required)]
    pub fuzz1116: Vec<rerun::testing::components::AffixFuzzer16>,

    #[rerun(component_required)]
    pub fuzz1117: Vec<rerun::testing::components::AffixFuzzer17>,

    #[rerun(component_required)]
    pub fuzz1118: Vec<rerun::testing::components::AffixFuzzer18>,

    #[rerun(component_required)]
    pub fuzz1122: Vec<rerun::testing::components::AffixFuzzer22>,
}

#[rerun::rerun_type]
#[rust(derive(PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer3 {
    #[rerun(component_optional)]
    pub fuzz2001: Option<rerun::testing::components::AffixFuzzer1>,

    #[rerun(component_optional)]
    pub fuzz2002: Option<rerun::testing::components::AffixFuzzer2>,

    #[rerun(component_optional)]
    pub fuzz2003: Option<rerun::testing::components::AffixFuzzer3>,

    #[rerun(component_optional)]
    pub fuzz2004: Option<rerun::testing::components::AffixFuzzer4>,

    #[rerun(component_optional)]
    pub fuzz2005: Option<rerun::testing::components::AffixFuzzer5>,

    #[rerun(component_optional)]
    pub fuzz2006: Option<rerun::testing::components::AffixFuzzer6>,

    #[rerun(component_optional)]
    pub fuzz2007: Option<rerun::testing::components::AffixFuzzer7>,

    #[rerun(component_optional)]
    pub fuzz2008: Option<rerun::testing::components::AffixFuzzer8>,

    #[rerun(component_optional)]
    pub fuzz2009: Option<rerun::testing::components::AffixFuzzer9>,

    #[rerun(component_optional)]
    pub fuzz2010: Option<rerun::testing::components::AffixFuzzer10>,

    #[rerun(component_optional)]
    pub fuzz2011: Option<rerun::testing::components::AffixFuzzer11>,

    #[rerun(component_optional)]
    pub fuzz2012: Option<rerun::testing::components::AffixFuzzer12>,

    #[rerun(component_optional)]
    pub fuzz2013: Option<rerun::testing::components::AffixFuzzer13>,

    #[rerun(component_optional)]
    pub fuzz2014: Option<rerun::testing::components::AffixFuzzer14>,

    #[rerun(component_optional)]
    pub fuzz2015: Option<rerun::testing::components::AffixFuzzer15>,

    #[rerun(component_optional)]
    pub fuzz2016: Option<rerun::testing::components::AffixFuzzer16>,

    #[rerun(component_optional)]
    pub fuzz2017: Option<rerun::testing::components::AffixFuzzer17>,

    #[rerun(component_optional)]
    pub fuzz2018: Option<rerun::testing::components::AffixFuzzer18>,
}

#[rerun::rerun_type]
#[rust(derive(PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer4 {
    #[rerun(component_optional)]
    pub fuzz2101: Option<Vec<rerun::testing::components::AffixFuzzer1>>,

    #[rerun(component_optional)]
    pub fuzz2102: Option<Vec<rerun::testing::components::AffixFuzzer2>>,

    #[rerun(component_optional)]
    pub fuzz2103: Option<Vec<rerun::testing::components::AffixFuzzer3>>,

    #[rerun(component_optional)]
    pub fuzz2104: Option<Vec<rerun::testing::components::AffixFuzzer4>>,

    #[rerun(component_optional)]
    pub fuzz2105: Option<Vec<rerun::testing::components::AffixFuzzer5>>,

    #[rerun(component_optional)]
    pub fuzz2106: Option<Vec<rerun::testing::components::AffixFuzzer6>>,

    #[rerun(component_optional)]
    pub fuzz2107: Option<Vec<rerun::testing::components::AffixFuzzer7>>,

    #[rerun(component_optional)]
    pub fuzz2108: Option<Vec<rerun::testing::components::AffixFuzzer8>>,

    #[rerun(component_optional)]
    pub fuzz2109: Option<Vec<rerun::testing::components::AffixFuzzer9>>,

    #[rerun(component_optional)]
    pub fuzz2110: Option<Vec<rerun::testing::components::AffixFuzzer10>>,

    #[rerun(component_optional)]
    pub fuzz2111: Option<Vec<rerun::testing::components::AffixFuzzer11>>,

    #[rerun(component_optional)]
    pub fuzz2112: Option<Vec<rerun::testing::components::AffixFuzzer12>>,

    #[rerun(component_optional)]
    pub fuzz2113: Option<Vec<rerun::testing::components::AffixFuzzer13>>,

    #[rerun(component_optional)]
    pub fuzz2114: Option<Vec<rerun::testing::components::AffixFuzzer14>>,

    #[rerun(component_optional)]
    pub fuzz2115: Option<Vec<rerun::testing::components::AffixFuzzer15>>,

    #[rerun(component_optional)]
    pub fuzz2116: Option<Vec<rerun::testing::components::AffixFuzzer16>>,

    #[rerun(component_optional)]
    pub fuzz2117: Option<Vec<rerun::testing::components::AffixFuzzer17>>,

    #[rerun(component_optional)]
    pub fuzz2118: Option<Vec<rerun::testing::components::AffixFuzzer18>>,
}
