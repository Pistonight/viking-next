use std::ops::Deref;
use std::sync::Arc;

use colored::Colorize;
use cu::pre::*;
use itertools::Itertools;
use rayon::prelude::*;
use rustc_hash::FxHashMap;

use crate::demangle;
use crate::fl;
#[cfg(feature = "python")]
use crate::python;
use crate::python::pre::*;
use crate::ui;

/// List of all [`Info`]s
#[derive(Clone, Default, Constructor, Into, Debug, Deref, DerefMut)]
#[repr(transparent)]
pub struct InfoList(Vec<Info>);
impl InfoList {
    /// Create a map of offset to [`Info`]
    pub fn make_known_function_map(&self) -> FxHashMap<u32, fl::Info> {
        let mut known_functions =
            FxHashMap::with_capacity_and_hasher(self.len(), Default::default());
        for function in &self.0 {
            known_functions.insert(function.offset, function.clone());
        }
        known_functions
    }
    /// Create a map of name (label) to [`Info`]
    pub fn make_known_function_name_map(&self) -> FxHashMap<&str, fl::Info> {
        let mut known_functions =
            FxHashMap::with_capacity_and_hasher(self.len(), Default::default());

        for function in self.iter() {
            for label in function.label.names() {
                known_functions.insert(label.as_str(), function.clone());
            }
        }

        known_functions
    }

    /// Fuzzy search by `name`, then interactively select a result.
    pub fn interactive_fuzzy_select(&self, name: &str, description: &str) -> cu::Result<Info> {
        let candidates = self.fuzzy_search(name, true);
        self.interactive_select(name, &candidates, description)
    }

    /// Interactively select an [`Info`] from a list of candidates filtered
    /// from this list
    pub fn interactive_select(
        &self,
        query: &str,
        candidates: &[Info],
        description: &str,
    ) -> cu::Result<Info> {
        let match_word = if candidates.len() == 1 {
            "match"
        } else {
            "matches"
        };
        let prompt = format!(
            "{} is ambiguous in {} (found {} {match_word}); did you mean:",
            query.dimmed(),
            description.red(),
            candidates.len()
        );
        let selected = ui::interactive_select(&prompt, candidates, |info| {
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

    /// Find a function by name
    pub fn find_exact(&self, name: &str) -> Option<Info> {
        self.0
            .par_iter()
            .find_first(|function| function.name() == name)
            .cloned()
    }

    /// Fuzzy search symbol info given the query string. Return matched symbol infos.
    ///
    /// If `prefer_exact` is true, it will return just the exact match if there is one.
    pub fn fuzzy_search(&self, name: &str, prefer_exact: bool) -> Vec<Info> {
        if prefer_exact {
            if let Some(exact_match) = self.find_exact(name) {
                return vec![exact_match.clone()];
            }
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
}

/// Clone-on-write handle to an [`InfoData`]
#[derive(Default, Deserialize, Clone, Debug, Display)]
#[display("{:#?}", self.0)]
#[pyclass(from_py_object, str)]
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

    /// Clone a new copy of the underlying data
    pub fn clone_data(&self) -> InfoData {
        self.0.as_ref().clone()
    }
}

/// Information about an address
#[derive(Default, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct InfoData {
    /// The address as offset from the base address defined in the config
    pub offset: u32,
    /// The base address, loaded from config
    #[serde(skip)]
    pub address_base: u64,
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
        label: AddressLabel,
        status: Status,
    ) -> Self {
        Self {
            offset,
            address_base,
            size,
            label,
            status,
            lazy: false,
            guess: false,
        }
    }
}

impl InfoData {
    /// Return `true` if the function is not a library function and
    /// has been worked on (i.e. is not `U`)
    #[inline(always)]
    pub fn is_decompiled(&self) -> bool {
        !matches!(self.status, Status::NotDecompiled | Status::Library)
    }
    /// Get the name (first if multiple)
    #[inline(always)]
    pub fn name(&self) -> &str {
        self.label.name()
    }
    /// Get all names
    #[inline(always)]
    pub fn names(&self) -> &[String] {
        self.label.names()
    }
    /// Get the (absolute) start address in the original ELF
    #[inline(always)]
    pub fn get_start(&self) -> u64 {
        self.address_base + self.offset as u64
    }
    /// Get the (absolute) end address in the original ELF
    #[inline(always)]
    pub fn get_end(&self) -> u64 {
        self.get_start() + self.size as u64
    }
}

#[rustfmt::skip]
#[cfg(feature = "python")]
#[pymethods]
impl Info {
    /// Create a new empty `Info` with the given names
    #[new]
    fn py_new(state: &python::State, names: Vec<String>) -> PyResult<Self> {
        let label = python::into_pyresult(AddressLabel::new(names))?;
        let info = InfoData {
            label,
            address_base: state.config.repo.address_base,
            ..Default::default()
        };
        Ok(info.into())
    }

    /// The address as offset from the base address defined in the config
    #[getter(offset)] fn py_offset(&self) -> u32 { self.offset }
    /// Set the offset from the base address
    #[setter(offset)] fn py_set_offset(&mut self, offset: u32) { self.data_mut().offset = offset }
    /// Size of the symbol at the address
    #[getter(size)] fn py_size(&self) -> u32 { self.size }
    /// Set the size
    #[setter(size)] fn py_set_size(&mut self, size: u32) { self.data_mut().size = size }
    /// Decompilation status of the symbol
    #[getter(status)] fn py_status(&self) -> Status { self.status }
    /// Set the decompilation status of the symbol
    #[setter(status)] fn py_set_status(&mut self, status: Status) { self.data_mut().status = status }
    /// If the symbol's placement into a TU is lazy. Typically marked with V or W
    /// in the output of llvm-nm.
    #[getter(lazy)] fn py_lazy(&self) -> bool { self.lazy }
    /// Set if the symbol is lazy
    #[setter(lazy)] fn py_set_lazy(&mut self, lazy: bool) { self.data_mut().lazy = lazy }
    /// If the symbol name is a guess instead of from some string in the original binary
    #[getter(guess)] fn py_guess(&self) -> bool { self.guess }
    /// Set if the symbol is a guess
    #[setter(guess)] fn py_set_guess(&mut self, guess: bool) { self.data_mut().guess = guess }
    /// Return `true` if the function is not a library function and
    /// has been worked on (i.e. is not `U`)
    #[getter(is_decompiled)] fn py_is_decompiled(&self) -> bool { self.is_decompiled() }
    /// Get the (absolute) start address in the original ELF
    #[getter(start)] fn py_get_start(&self) -> u64 { self.get_start() }
    /// Get the (absolute) end address in the original ELF
    #[getter(end)] fn py_get_end(&self) -> u64 { self.get_end() }

    // -- label (name) bindings --

    /// Get the number of names
    #[getter(names_len)] fn py_names_len(&self) -> usize { self.names().len() }
    /// Get the name (first if multiple)
    #[getter(name)] fn py_name(&self) -> String { self.name().to_string() }
    /// Get all names
    #[getter(names)] fn py_names(&self) -> Vec<String> { self.names().to_vec() }

    /// Set the names. Raises exception if there are more than one name and any of the name is
    /// empty.
    #[pyo3(name = "set_names")]
    fn py_set_names(&mut self, names: Vec<String>) -> PyResult<()> {
        let label = python::into_pyresult(AddressLabel::new(names))?;
        self.data_mut().label = label;
        Ok(())
    }
}

/// Label for an address
///
/// Each `AddressLabel` is guaranteed to hold at least one name.
/// It may hold exactly one name that can be empty or non-empty,
/// but if it holds more than one name, then every name must be non-empty.
///
/// A function might have multiple name when the compiler merges
/// the C1/C2 constructors or D1/D2 destructors
#[derive(Clone, Debug, PartialEq, Eq, Display)]
#[display("{:?}", self.0)]
#[repr(transparent)]
pub struct AddressLabel(Vec<String>);
impl Default for AddressLabel {
    fn default() -> Self {
        Self(vec![Default::default()])
    }
}
impl AddressLabel {
    /// Create a new address label from a list of names, checking the invariants
    pub fn new(names: Vec<String>) -> cu::Result<Self> {
        match names.len() {
            0 => Ok(Default::default()),
            1 => Ok(Self::new_single(names.into_iter().next().unwrap())),
            _ => Self::new_many(names)
        }
    }
    /// Create a new address label from a list of names, without validation
    pub fn new_unchecked(name: Vec<String>) -> Self {
        Self(name)
    }
    /// Create a new address label with a single name
    pub fn new_single(name: impl Into<String>) -> Self {
        Self(vec![name.into()])
    }
    /// Create a new address label with many names
    pub fn new_many(name: impl IntoIterator<Item = impl Into<String>>) -> cu::Result<Self> {
        let name = name.into_iter().map(Into::into).collect_vec();
        if name.len() > 1 {
            for n in &name {
                if n.is_empty() {
                    cu::bail!("AddressLabel with more than one names cannot have empty names")
                }
            }
        }
        if name.is_empty() {
            cu::bail!("AddressLabel must have at least one name")
        }

        Ok(Self(name))
    }
    /// Get the name (first if it has multiple names)
    pub fn name(&self) -> &str {
        // unwrap: AddressLabel invariant
        self.0.first().unwrap()
    }

    /// Get all names
    pub fn names(&self) -> &[String] {
        &self.0
    }
}

#[allow(clippy::clone_on_copy)] // pyo3 didn't suppress warnings in there generated code
mod status_imp {
    use super::*;
    /// Decompilation status of a function/symbol
    #[derive(Default, Clone, Copy, Serialize, Deserialize, Debug, Display, PartialEq, Eq)]
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
        #[default]
        NotDecompiled,
        /// Work in progress
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
#[cfg(feature = "python")]
#[pymethods]
impl Status {
    /// Parse status string into enum
    #[pyo3(name = "from_name")]
    #[staticmethod]
    fn py_from_name(s: &str) -> PyResult<Self> { python::into_pyresult(Self::from_name(s)) }
    /// Parse status code into enum
    #[pyo3(name = "from_code")]
    #[staticmethod]
    fn py_from_code(s: &str) -> PyResult<Self> { python::into_pyresult(Self::from_code(s)) }
    /// Get the single-character code representation
    #[pyo3(name = "code")]
    fn py_code(&self) -> char { self.code() }
    /// Get descriptive string
    #[pyo3(name = "description")]
    fn py_description(&self) -> &'static str { self.description() }
}
