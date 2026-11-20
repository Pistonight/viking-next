use std::fmt::Write;
use std::path::Path;

use cu::pre::*;
use rustc_hash::FxHashSet;

use crate::config::{self, Config};
use crate::fl;
#[cfg(feature = "python")]
use crate::python;
use crate::python::pre::*;

/// Load the file list from a path, assuming YAML format
#[inline(always)]
pub fn load_file_list_yml_at(config: &Config, path: impl AsRef<Path>) -> cu::Result<FileList> {
    load_file_list_yml_at_imp(config, path.as_ref())
}
fn load_file_list_yml_at_imp(config: &Config, path: &Path) -> cu::Result<FileList> {
    #[derive(Deserialize)]
    #[serde(transparent)]
    struct FileListWrapper(#[serde(with = "tuple_vec_map")] Vec<(String, Object)>);

    let reader = cu::fs::reader(path)?;
    let FileListWrapper(data) = yaml::read(reader)?;
    FileList::new(config, data)
}

/// Write the file list data to `path`, assuming YAML format.
///
/// Note you can pass in a reference of [`fl::FileList`] directly because it implements `Deref`.
#[inline(always)]
pub fn write_file_list_yml_at(
    path: impl AsRef<Path>,
    file_list_data: &[(String, Object)],
) -> cu::Result<()> {
    write_file_list_yml_at_imp(path.as_ref(), file_list_data)
}
fn write_file_list_yml_at_imp(path: &Path, file_list_data: &[(String, Object)]) -> cu::Result<()> {
    let mut buf = String::new();
    for (name, obj) in file_list_data {
        writeln!(&mut buf, "{}:", fl::quote_yaml_string_if_needed(name))?;
        obj.fmt_file_list_yml(&mut buf)?;
    }
    cu::fs::write(path, buf)?;
    Ok(())
}

/// The YAML File List structure
#[derive(Debug, Clone, Deref)]
#[pyclass(from_py_object)]
pub struct FileList {
    #[deref]
    data: Vec<(String, Object)>,
    // this is not public because mutating this list will not
    // save it into the data map
    info_list: fl::InfoList,
}

impl FileList {
    /// Create a new file list from in-memory list of objects
    pub fn new(config: &Config, mut objects: Vec<(String, Object)>) -> cu::Result<Self> {
        for (_, object) in &mut objects {
            cu::ensure!(
                !object.text_section.is_empty(),
                "an object in the file list must have a non-empty .text section"
            )?;
            object.text_section.sort_by_key(|x| x.offset);
            for info in &mut object.text_section {
                info.data_mut().address_base = config.repo.address_base;
            }
        }
        objects.sort_by_key(|(_, obj)| obj.offset());

        let mut file_list = FileList {
            data: objects,
            info_list: fl::InfoList::default(),
        };

        let num_names = file_list.reload_info_list();
        file_list.validate(num_names)?;
        Ok(file_list)
    }

    /// Convert a flat info list into a file list, putting everything in a single "UNKNOWN" file
    pub fn from_info_list(info_list: fl::InfoList) -> Self {
        let unknown_object = Object {
            text_section: info_list.clone().into(),
        };
        Self {
            data: vec![(config::FILE_LIST_UNKNOWN_FILE.to_string(), unknown_object)],
            info_list,
        }
    }
    /// Perform checks
    #[cu::context("failed to validate the file list")]
    pub fn validate(&self, num_names: usize) -> cu::Result<()> {
        cu::check!(
            self.check_names(num_names),
            "found issues with names in the file list"
        )?;
        cu::check!(
            self.check_for_overlapping_functions(),
            "found overlapping functions in the file list"
        )?;
        Ok(())
    }
    fn check_names(&self, num_names: usize) -> cu::Result<()> {
        let mut seen_names = if num_names == 0 {
            Default::default()
        } else {
            FxHashSet::with_capacity_and_hasher(num_names, Default::default())
        };
        let mut duplicates = Vec::new();

        for entry in self.info_list.iter() {
            let start = entry.get_start();
            let is_decompiled = entry.is_decompiled();
            if is_decompiled && entry.label.name().is_empty() {
                cu::bail!("function at 0x{start:016x} is marked as O/M/m but has an empty name",);
            }
            for n in entry.label.names() {
                if !n.is_empty() && !seen_names.insert(n) {
                    duplicates.push(n);
                }
            }
        }

        if !duplicates.is_empty() {
            cu::bail!("found duplicates: {:#?}", duplicates);
        }

        Ok(())
    }

    fn check_for_overlapping_functions(&self) -> cu::Result<()> {
        for pair in self.info_list.windows(2) {
            let first = &pair[0];
            let second = &pair[1];
            cu::ensure!(
                first.get_start() < second.get_start() && first.get_end() <= second.get_start(),
                "overlapping functions: {:016x} - {:016x} and {:016x} - {:016x}",
                first.get_start(),
                first.get_end(),
                second.get_start(),
                second.get_end(),
            )?;
        }

        Ok(())
    }

    /// Search for the object whose name matches the input
    pub fn object_from_name(&self, name: &str) -> Option<&Object> {
        let (_, object) = self.data.iter().find(|(n, _)| n == name)?;
        Some(object)
    }

    /// Search for the object whose name matches the input
    pub fn object_from_name_mut(&mut self, name: &str) -> Option<(&mut String, &mut Object)> {
        let (name, object) = self.data.iter_mut().find(|(n, _)| n == name)?;
        Some((name, object))
    }

    /// Get the name and object data for the object containing the offset
    ///
    /// If the offset is larger than all objects, the last object is returned.
    pub fn object_from_offset(&self, offset: u32) -> Option<(&str, &Object)> {
        let (name, object) = &self.data[self.object_index_from_offset(offset)?];
        Some((name, object))
    }

    /// Get the name and object data for the object containing the offset
    ///
    /// If the offset is larger than all objects, the last object is returned.
    pub fn object_from_offset_mut(&mut self, offset: u32) -> Option<(&mut String, &mut Object)> {
        let i = self.object_index_from_offset(offset)?;
        let (name, object) = &mut self.data[i];
        Some((name, object))
    }

    /// Get the index of the object containing the offset
    ///
    /// If the offset is larger than all objects, the last object is returned.
    pub fn object_index_from_offset(&self, offset: u32) -> Option<usize> {
        match self
            .data
            .binary_search_by_key(&offset, |(_, obj)| obj.offset())
        {
            Ok(i) => Some(i),
            Err(i) => {
                // if i == 0 then the offset to search for is less than
                // the first object
                Some(i.checked_sub(1)?)
            }
        }
    }

    /// Borrow the flat list of [`fl::Info`]
    pub fn info_list(&self) -> &fl::InfoList {
        &self.info_list
    }

    /// Reload the in-memory info list to be synced with the file list data
    pub fn reload_info_list(&mut self) -> usize {
        let mut num_names = 0;
        let mut info_list = Vec::with_capacity(config::FUNCTION_LEN_UPPER_BOUND);
        for (_, object) in &self.data {
            for info in &object.text_section {
                num_names += info.names().len();
            }
            info_list.extend(object.text_section.iter().cloned());
        }
        info_list.sort_by_key(|x| x.offset);
        self.info_list = fl::InfoList::new(info_list);

        num_names
    }

    /// Update a single symbol at offset
    ///
    /// The update will NOT be reflected in the in-memory flattened function list (info_list).
    /// You must call [`reload_info_list`](`Self::reload_info_list`) to update it.
    pub fn update<F: FnOnce(&mut fl::InfoData)>(
        &mut self,
        offset: u32,
        update_fn: F,
    ) -> cu::Result<()> {
        if let Some(i) = self.object_index_from_offset(offset) {
            let object = &mut self.data[i].1;
            for function in object.text_section.iter_mut() {
                if function.offset == offset {
                    update_fn(function.data_mut());
                    return Ok(());
                }
            }
        }
        cu::bail!("could not find function to update: offset = 0x{offset:08x}");
    }
}

#[cfg(feature = "python")]
#[pymethods]
impl FileList {
    #[new]
    fn py_new(state: &python::State, data: Vec<(String, Object)>) -> PyResult<Self> {
        python::into_pyresult(Self::new(&state.config, data))
    }
    /// Number of objects
    #[getter(objects_len)]
    fn py_objects_len(&self) -> usize {
        self.data.len()
    }

    /// Get a list of all objects and their name, sorted by offset.
    ///
    /// The list is a copy. Modifying the list will not change this FileList.
    #[pyo3(name = "get_all_objects")]
    fn py_objects(&self) -> Vec<(String, Object)> {
        self.data.clone()
    }

    /// Get a flat list of all `Info`.
    ///
    /// The underlying `Vec` that holds shared `Info` pointers is cloned to Python.
    /// The underlying `Info`s will only be cloned on write. Modifying the returned list will not
    /// modify the file list.
    ///
    /// To modify the FileList, use the mutater methods (`set_*` and `update_*`)
    #[pyo3(name = "info_list")]
    fn py_info_list(&mut self) -> Vec<fl::Info> {
        // ensure synced, sacrifice some perf to get rid of foot-guns in Python
        self.reload_info_list();
        self.info_list().clone().into()
    }

    /// Search for the object containing the offset and return its index.
    ///
    /// `None` is returned if the offset is less than the first object.
    /// If the offset is larger than the last object, the last object is returned.
    #[pyo3(name = "object_index_from_offset")]
    fn py_object_index_from_offset(&self, offset: u32) -> Option<usize> {
        self.object_index_from_offset(offset)
    }

    /// Get the name and data of the `i`-th object. Raises exception if index is out of bound.
    ///
    /// The returned object is a copy. To modify the FileList, use the mutater methods (`set_*` and `update_*`)
    #[pyo3(name = "ith_object")]
    fn py_ith_object(&self, index: usize) -> PyResult<(String, Object)> {
        let o = python::into_pyresult(cu::check!(
            self.get(index),
            "index out of bound for the list of objects: index={index}, length={}",
            self.len()
        ))?;
        Ok(o.clone())
    }

    /// Search for the object containing the offset and return its name and data.
    ///
    /// The returned object is a copy. To modify the FileList, use the mutater methods (`set_*` and `update_*`)
    #[pyo3(name = "object_from_offset")]
    fn py_object_from_offset(&self, offset: u32) -> Option<(String, Object)> {
        let (name, object) = self.object_from_offset(offset)?;
        Some((name.to_string(), object.clone()))
    }

    /// Search for the object with the name and return its data.
    ///
    /// The returned object is a copy. To modify the FileList, use the mutater methods (`set_*` and `update_*`)
    #[pyo3(name = "object_from_name")]
    fn py_object_from_name(&self, name: &str) -> Option<Object> {
        self.object_from_name(name).cloned()
    }

    /// Set the data of the `i`-th object.
    #[pyo3(name = "set_ith_object")]
    fn py_set_ith_object(&mut self, index: usize, object: Object) -> PyResult<()> {
        let len = self.len();
        let o = python::into_pyresult(cu::check!(
            self.data.get_mut(index),
            "index out of bound for the list of objects: index={index}, length={len}"
        ))?;
        o.1 = object;
        self.data.sort_by_key(|(_, obj)| obj.offset());
        Ok(())
    }

    /// Set the name of the `i`-th object.
    #[pyo3(name = "set_ith_object_name")]
    fn py_set_ith_object_name(&mut self, index: usize, name: &str) -> PyResult<()> {
        let len = self.len();
        let o = python::into_pyresult(cu::check!(
            self.data.get_mut(index),
            "index out of bound for the list of objects: index={index}, length={len}"
        ))?;
        o.0 = name.to_string();
        Ok(())
    }

    /// Set the data of the object with the given name
    #[pyo3(name = "set_object_by_name")]
    fn py_set_object_by_name(&mut self, name: &str, object: Object) -> PyResult<()> {
        let (_, obj) = python::into_pyresult(cu::check!(
            self.object_from_name_mut(name),
            "cannot find object with name {name:?}"
        ))?;
        *obj = object;
        self.data.sort_by_key(|(_, obj)| obj.offset());
        Ok(())
    }

    /// Set the new name of the object with the given name
    #[pyo3(name = "rename_object")]
    fn py_rename_object(&mut self, name: &str, new_name: &str) -> PyResult<()> {
        let (name, _) = python::into_pyresult(cu::check!(
            self.object_from_name_mut(name),
            "cannot find object with name {name:?}"
        ))?;
        *name = new_name.to_string();
        Ok(())
    }

    /// Update a symbol data at the offset. Raises an exception if no symbol is found
    /// with the offset.
    #[pyo3(name = "update_at_offset")]
    fn py_update_at_offset(&mut self, offset: u32, data: fl::Info) -> PyResult<()> {
        python::into_pyresult(self.update(offset, |x| *x = data.clone_data()))
    }

    // #[pyo3(name = "")
    // fn py_ith_object()
}

/// One object entry in a YAML file list
#[derive(Deserialize, Clone, Debug, Display)]
#[display("<Object @ offset=0x{:08x}>", self.offset())]
#[pyclass(from_py_object, str)]
pub struct Object {
    // -- Vecs are not #[pyo3(get)] to avoid foot-guns
    // since each time they are accessed, the whole vec is cloned to Python,
    // and modifying them in Python does not change the copy in the Rust object.
    /// List of function infos
    #[serde(rename(serialize = ".text", deserialize = ".text"))]
    pub text_section: Vec<fl::Info>,
}
impl Object {
    /// Get the start offset of this object in the original ELF
    pub fn offset(&self) -> u32 {
        // unwrap: validated when loading FileList
        self.text_section.first().unwrap().offset
    }
}
#[rustfmt::skip]
#[cfg(feature = "python")]
#[pymethods]
impl Object {
    /// Create a new object entry. The input data is not validated.
    #[new] fn py_new(text_section: Vec<fl::Info>) -> PyResult<Self> {
        if text_section.is_empty() {
            return python::into_pyresult(Err(cu::fmterr!("object entry must contain at least one function")));
        }
        Ok(Self { text_section })
    }
    /// The start offset of the object in the original ELF
    #[getter(offset)] fn py_offset(&self) -> u32 { self.offset() }

    /// Get the text section (list of function infos).
    ///
    /// The underlying `Vec` that holds shared `Info` pointers is cloned to Python.
    /// The underlying `Info`s will only be cloned on write. Modifying the returned list will not
    /// modify this `Object`. Use `set_text_section` for that.
    #[pyo3(name = "get_text_section")]
    fn py_get_text_section(&self) -> Vec<fl::Info> {
        self.text_section.clone()
    }

    /// Set the text section (list of function infos)
    #[pyo3(name = "set_text_section")]
    fn py_set_text_section(&mut self, mut text_section: Vec<fl::Info>) -> PyResult<()> {
        if text_section.is_empty() {
            return python::into_pyresult(Err(cu::fmterr!("object entry must contain at least one function")));
        }
        text_section.sort_by_key(|x| x.offset);
        self.text_section = text_section;
        Ok(())
    }
}
