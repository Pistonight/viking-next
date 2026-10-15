use std::fmt::Write;
use std::ops::Deref;
use std::sync::Arc;

use cu::pre::*;
use rayon::prelude::*;
use rustc_hash::FxHashMap;

use crate::config::Config;
use crate::demangle;
use crate::fl;
use crate::ui;

#[derive(Clone, Constructor, Into, Debug, Deref, DerefMut)]
#[repr(transparent)]
pub struct InfoList(Vec<Info>);
impl InfoList {
    pub fn make_known_function_map(&self) -> FxHashMap<u32, fl::Info> {
        let mut known_functions =
            FxHashMap::with_capacity_and_hasher(self.len(), Default::default());
        for function in &self.0 {
            known_functions.insert(function.offset, function.clone());
        }
        known_functions
    }
    pub fn interactive_fuzzy_select(&self, name: &str) -> cu::Result<Info> {
        let candidates = self.fuzzy_search(name);
        let selected = ui::interactive_select(name, &candidates, |info| {
            ui::format_symbol_name(info.name())
        })?;
        match selected {
            None => cu::bail!("no match for {}", ui::format_symbol_name(name)),
            Some(info) => Ok(info.clone()),
        }
    }
    /// Fuzzy search symbol info given the query string. Return matched symbol infos
    pub fn fuzzy_search(&self, name: &str) -> Vec<Info> {
        let exact_match = self
            .0
            .par_iter()
            .find_first(|function| function.name() == name);

        if let Some(exact_match) = exact_match {
            return vec![exact_match.clone()];
        }

        // Find all functions whose demangled name contains the specified string.
        // This is more expensive than a simple string comparison, so only do this after
        // we have failed to find an exact match.
        let mut candidates: Vec<Info> = self
            .0
            .par_iter()
            .filter(|function| {
                demangle::demangle_str(function.name())
                    .is_ok_and(|demangled| demangled.contains(name))
                    || function.name().contains(name)
            })
            .cloned()
            .collect();

        candidates.sort_by_key(|info| info.offset);
        candidates
    }

    /// Format into YAML file list writer
    pub fn fmt_file_list_yml<W: Write>(&self, mut out: W) -> std::fmt::Result {
        for info in &self.0 {
            info.fmt_file_list_yml(&mut out)?;
        }
        Ok(())
    }
}

/// Clone-on-write Handle to an InfoData
#[derive(Deserialize, Clone, Debug)]
#[serde(transparent)]
pub struct Info(Arc<InfoData>);
impl Deref for Info {
    type Target = InfoData;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl From<InfoData> for Info {
    fn from(value: InfoData) -> Self {
        Self(Arc::new(value))
    }
}
impl Info {
    /// Update the info, cloning to a new copy if the underlying data
    /// is currently shared. AsMut/DerefMut is not implemented so
    /// caller is aware of the clone-on-write nature.
    pub fn data_mut(&mut self) -> &mut InfoData {
        Arc::make_mut(&mut self.0)
    }
}

/// Information about an address
#[derive(Deserialize, Clone, Debug)]
pub struct InfoData {
    /// The address as offset from the base address defined in the config
    #[serde(serialize_with = "as_hex")]
    pub offset: u32,
    /// The offset plus the base address
    #[serde(skip)]
    pub absolute_address: u64,
    /// Size of the symbol at the address
    pub size: u32,
    /// Name(s) of the symbol at the address
    pub label: AddressLabel,
    /// Decompilation status of the symbol
    pub status: Status,
    /// If the symbol's placement into a TU is lazy. Typically marked with V or W
    /// in the output of llvm-nm.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lazy: bool,
    /// If the symbol name is a guess instead of from some string in the original binary
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub guess: bool,
}
impl InfoData {
    pub fn new(
        address_base: u64,
        offset: u32,
        size: u32,
        label: impl Into<AddressLabel>,
        status: Status,
    ) -> Self {
        Self {
            offset,
            absolute_address: offset as u64 + address_base,
            size,
            label: label.into(),
            status,
            lazy: false,
            guess: false,
        }
    }
    /// Format into YAML file list writer
    pub fn fmt_file_list_yml<W: Write>(&self, mut out: W) -> std::fmt::Result {
        let prefix = "  ";
        writeln!(out, "{prefix}- offset: 0x{:08x}", self.offset)?;
        writeln!(out, "{prefix}  size: {}", self.size)?;
        match &self.label {
            AddressLabel::Single(label) => {
                writeln!(
                    out,
                    "{prefix}  label: {}",
                    fl::quote_yaml_string_if_needed(label)
                )?;
            }
            AddressLabel::Multi(labels) => {
                writeln!(out, "{prefix}  label:")?;
                for l in labels {
                    writeln!(out, "{prefix}  - {}", fl::quote_yaml_string_if_needed(l))?;
                }
            }
        }
        writeln!(
            out,
            "{prefix}  status: {}",
            yaml::stringify(&self.status).unwrap()
        )?;
        if self.lazy {
            writeln!(out, "{prefix}  lazy: true")?;
        }
        if self.guess {
            writeln!(out, "{prefix}  guess: true")?;
        }
        Ok(())
    }
}

/// Label for an address
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum AddressLabel {
    /// Address has a single name
    Single(String),
    /// Adderss has multiple names.
    ///
    /// For example, constructor C1/C2 or destructor D1/D2
    /// are usually the same function.
    Multi(Vec<String>),
}
impl From<String> for AddressLabel {
    fn from(value: String) -> Self {
        Self::Single(value)
    }
}
impl From<Vec<String>> for AddressLabel {
    fn from(value: Vec<String>) -> Self {
        Self::Multi(value)
    }
}

impl InfoData {
    pub fn is_decompiled(&self) -> bool {
        !matches!(self.status, Status::NotDecompiled | Status::Library)
    }
    pub fn num_names(&self) -> usize {
        match &self.label {
            AddressLabel::Single(_) => 1,
            AddressLabel::Multi(labels) => labels.len(),
        }
    }
    pub fn name(&self) -> &str {
        match &self.label {
            AddressLabel::Single(label) => label,
            // unwrap: when FileList is loaded, it is validated to be non-empty
            AddressLabel::Multi(labels) => labels.first().unwrap(),
        }
    }

    pub fn get_start(&self) -> u64 {
        self.absolute_address
    }

    pub fn get_end(&self) -> u64 {
        self.get_start() + self.size as u64
    }
}

/// Decompilation status of a function/symbol
#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum Status {
    /// Decompiled and matching
    Matching,
    /// Decompiled and not matching due to minor mismatches
    NonMatchingMinor,
    /// Decompiled and not matching due to major mismatches
    NonMatchingMajor,
    /// Unnamed or not decompiled
    NotDecompiled,
    /// Working in progress
    Wip,
    /// Library function - out of scope for decompilation
    Library,
}

impl Status {
    pub fn from_str(s: &str) -> cu::Result<Self> {
        Ok(match s.trim() {
            "m" => Self::NonMatchingMinor,
            "M" => Status::NonMatchingMajor,
            "O" => Status::Matching,
            "U" => Status::NotDecompiled,
            "W" => Status::Wip,
            "L" => Status::Library,
            other => cu::bail!("unexpected status code: {other}"),
        })
    }
    pub fn code(self) -> char {
        match self {
            Status::Matching => 'O',
            Status::NonMatchingMinor => 'm',
            Status::NonMatchingMajor => 'M',
            Status::NotDecompiled => 'U',
            Status::Wip => 'W',
            Status::Library => 'L',
        }
    }
    pub fn description(&self) -> &'static str {
        match &self {
            Status::Matching => "matching",
            Status::NonMatchingMinor => "non-matching (minor)",
            Status::NonMatchingMajor => "non-matching (major)",
            Status::NotDecompiled => "not decompiled",
            Status::Wip => "WIP",
            Status::Library => "library function",
        }
    }
}
