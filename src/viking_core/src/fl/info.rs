use std::fmt::Write;
use std::ops::Deref;
use std::sync::Arc;

use colored::Colorize;
use cu::pre::*;
use pyo3::prelude::*;
use rayon::prelude::*;
use rustc_hash::FxHashMap;

use crate::demangle;
use crate::fl;
use crate::interop;
use crate::ui;

#[derive(Clone, Default, Constructor, Into, Debug, Deref, DerefMut)]
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
    pub fn make_known_function_name_map(&self) -> FxHashMap<&str, fl::Info> {
        let mut known_functions =
            FxHashMap::with_capacity_and_hasher(self.len(), Default::default());

        for function in self.iter() {
            match &function.label {
                fl::AddressLabel::Single(label) => {
                    if label.is_empty() {
                        continue;
                    }
                    known_functions.insert(label.as_str(), function.clone());
                }
                fl::AddressLabel::Multi(labels) => {
                    for label in labels {
                        known_functions.insert(label.as_str(), function.clone());
                    }
                }
            }
        }

        known_functions
    }
    pub fn interactive_fuzzy_select(&self, name: &str, description: &str) -> cu::Result<Info> {
        let candidates = self.fuzzy_search(name);
        self.interactive_select(name, &candidates, description)
    }
    pub fn interactive_select(
        &self,
        query: &str,
        candidates: &[Info],
        description: &str,
    ) -> cu::Result<Info> {
        let selected = ui::interactive_select(query, description, candidates, |info| {
            ui::format_symbol_name(info.name())
        })?;
        match selected {
            None => cu::bail!(
                "no match for {} in {}",
                ui::format_symbol_name(query),
                description.red()
            ),
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
            .filter(|function| demangle::raw_or_demangled_contains(function.name(), name))
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
    pub fn clone_data(&self) -> InfoData {
        self.0.as_ref().clone()
    }
}

/// Information about an address
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
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

impl InfoData {
    #[inline(always)]
    pub fn is_decompiled(&self) -> bool {
        !matches!(self.status, Status::NotDecompiled | Status::Library)
    }
    /// Get the number of names
    #[inline(always)]
    pub fn num_names(&self) -> usize {
        self.label.len()
    }

    /// Get the name (first if multiple)
    #[inline(always)]
    pub fn name(&self) -> &str {
        self.label.name()
    }

    /// Get all names
    #[inline(always)]
    pub fn all_names(&self) -> Vec<String> {
        self.label.all()
    }

    /// Check if any name of the function entry matches the input name
    #[inline(always)]
    pub fn contains_name(&self, name: &str) -> bool {
        self.label.contains_name(name)
    }

    /// Get the (absolute) start address in the original ELF
    #[inline(always)]
    pub fn get_start(&self) -> u64 {
        self.absolute_address
    }

    /// Get the (absolute) end address in the original ELF
    #[inline(always)]
    pub fn get_end(&self) -> u64 {
        self.get_start() + self.size as u64
    }
}

/// Label for an address
#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Display)]
#[display("{self:#?}")]
#[pyclass(from_py_object, str)]
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
#[pymethods]
impl AddressLabel {
    /// Get the number of names
    #[allow(clippy::len_without_is_empty)] // it's guaranteed to be not empty
    pub fn len(&self) -> usize {
        match self {
            Self::Single(_) => 1,
            Self::Multi(labels) => labels.len(),
        }
    }

    /// Get the name (first if multiple)
    pub fn name(&self) -> &str {
        match self {
            Self::Single(label) => label,
            // unwrap: when FileList is loaded, it is validated to be non-empty
            Self::Multi(labels) => labels.first().unwrap(),
        }
    }

    /// Get all names
    pub fn all(&self) -> Vec<String> {
        match self {
            Self::Single(label) => vec![label.to_string()],
            Self::Multi(labels) => labels.clone(),
        }
    }

    /// Check if any name matches the input name
    pub fn contains_name(&self, name: &str) -> bool {
        match &self {
            Self::Single(label) => label == name,
            Self::Multi(labels) => labels.iter().any(|x| x == name),
        }
    }
}

#[allow(clippy::clone_on_copy)] // pyo3 didn't suppress warnings in there generated code
mod status_imp {
    use super::*;
    /// Decompilation status of a function/symbol
    #[derive(Clone, Copy, Serialize, Deserialize, Debug, Display, PartialEq, Eq)]
    #[display("{}", self.description())]
    #[pyclass(from_py_object, str)]
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
}
pub use status_imp::Status;
impl Status {
    /// Parse status string into enum
    pub fn from_name(s: &str) -> cu::Result<Self> {
        Ok(match s.trim() {
            "NonMatchingMinor" => Self::NonMatchingMinor,
            "NonMatchingMajor" => Self::NonMatchingMajor,
            "Matching" => Self::Matching,
            "NotDecompiled" => Self::NotDecompiled,
            "Wip" => Self::Wip,
            "Library" => Self::Library,
            other => cu::bail!("unexpected status string: {other}"),
        })
    }
    /// Parse status code into enum
    pub fn from_code(s: &str) -> cu::Result<Self> {
        Ok(match s.trim() {
            "m" => Self::NonMatchingMinor,
            "M" => Self::NonMatchingMajor,
            "O" => Self::Matching,
            "U" => Self::NotDecompiled,
            "W" => Self::Wip,
            "L" => Self::Library,
            other => cu::bail!("unexpected status code: {other}"),
        })
    }
    /// Get the single-character code representation
    pub fn code(self) -> char {
        match self {
            Self::Matching => 'O',
            Self::NonMatchingMinor => 'm',
            Self::NonMatchingMajor => 'M',
            Self::NotDecompiled => 'U',
            Self::Wip => 'W',
            Self::Library => 'L',
        }
    }
    /// Get descriptive string
    pub fn description(self) -> &'static str {
        match self {
            Self::Matching => "matching",
            Self::NonMatchingMinor => "non-matching (minor)",
            Self::NonMatchingMajor => "non-matching (major)",
            Self::NotDecompiled => "not decompiled",
            Self::Wip => "WIP",
            Self::Library => "library function",
        }
    }
}

#[rustfmt::skip]
#[pymethods]
impl Status {
    /// Parse status string into enum
    #[pyo3(name = "from_name")]
    #[staticmethod]
    pub fn py_from_name(s: &str) -> PyResult<Self> { interop::into_pyresult(Self::from_name(s)) }
    /// Parse status code into enum
    #[pyo3(name = "from_code")]
    #[staticmethod]
    pub fn py_from_code(s: &str) -> PyResult<Self> { interop::into_pyresult(Self::from_code(s)) }
    /// Get the single-character code representation
    #[pyo3(name = "code")]
    pub fn py_code(&self) -> char { self.code() }
    /// Get descriptive string
    #[pyo3(name = "description")]
    pub fn py_description(&self) -> &'static str { self.description() }
}
