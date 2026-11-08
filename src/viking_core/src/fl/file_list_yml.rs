use std::fmt::Write;
use std::path::Path;

use cu::pre::*;
use rustc_hash::FxHashSet;

use crate::config::{self, Config};
use crate::fl;

pub fn load_yml_file_list(config: &Config, path: &Path) -> cu::Result<FileList> {
    #[derive(Deserialize)]
    #[serde(transparent)]
    struct FileListWrapper(#[serde(with = "tuple_vec_map")] Vec<(String, Object)>);

    let reader = cu::fs::reader(path)?;
    let FileListWrapper(mut data) = yaml::read(reader)?;
    for (_, object) in &mut data {
        cu::ensure!(
            !object.text_section.is_empty(),
            "an object in the file list must have a non-empty .text section"
        )?;
        object.text_section.sort_by_key(|x| x.offset);
        for info in &mut object.text_section {
            info.data_mut().absolute_address = config.repo.address_base + info.offset as u64;
        }
    }
    data.sort_by_key(|(_, obj)| obj.offset());

    let mut file_list = FileList {
        data,
        info_list: fl::InfoList::default(),
    };

    let num_names = file_list.reload_info_list();
    file_list.validate(num_names)?;
    Ok(file_list)
}

pub fn write_yml_file_list(path: &Path, file_list_data: &[(String, Object)]) -> cu::Result<()> {
    let mut buf = String::new();
    for (name, obj) in file_list_data {
        writeln!(&mut buf, "{}:", fl::quote_yaml_string_if_needed(name))?;
        obj.fmt_file_list_yml(&mut buf)?;
    }
    cu::fs::write(path, buf)?;
    Ok(())
}

#[derive(Debug, Deref)]
pub struct FileList {
    #[deref]
    data: Vec<(String, Object)>,
    // this is not public because mutating this list will not
    // save it into the data map
    info_list: fl::InfoList,
}

impl FileList {
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
            match &entry.label {
                fl::AddressLabel::Single(name) => {
                    if is_decompiled && name.is_empty() {
                        cu::bail!(
                            "function at 0x{start:016x} is marked as O/M/m but has an empty name",
                        );
                    }
                    if !name.is_empty() && !seen_names.insert(name) {
                        duplicates.push(name);
                    }
                }
                fl::AddressLabel::Multi(names) => {
                    cu::ensure!(
                        !names.is_empty(),
                        "multi-label list must not be empty; please use a single empty string for unnamed symbols"
                    )?;
                    for n in names {
                        cu::ensure!(
                            !n.is_empty(),
                            "multi-label list must not contain empty strings"
                        )?;
                        if !seen_names.insert(n) {
                            duplicates.push(n);
                        }
                    }
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

    /// Get the name of the object containing the offset
    ///
    /// If the offset is larger than all objects, the last object is returned.
    pub fn object_name_for(&self, offset: u32) -> Option<&str> {
        match self
            .data
            .binary_search_by_key(&offset, |(_, obj)| obj.offset())
        {
            Ok(i) => Some(&self.data[i].0),
            Err(i) => {
                // if i == 0 then the offset to search for is less than
                // the first object
                let i = i.checked_sub(1)?;
                Some(&self.data[i].0)
            }
        }
    }

    pub fn info_list(&self) -> &fl::InfoList {
        &self.info_list
    }

    /// Reload the in-memory info list to be synced with the file list data
    pub fn reload_info_list(&mut self) -> usize {
        let mut num_names = 0;
        let mut info_list = Vec::with_capacity(config::FUNCTION_LEN_UPPER_BOUND);
        for (_, object) in &self.data {
            for info in &object.text_section {
                num_names += info.num_names();
            }
            info_list.extend(object.text_section.iter().cloned());
        }
        info_list.sort_by_key(|x| x.offset);
        self.info_list = fl::InfoList::new(info_list);

        num_names
    }

    /// Update a single symbol at offset
    ///
    /// The update will NOT be reflected in the in-memory flatten function list (info_list)
    pub fn update<F: FnOnce(&mut fl::InfoData)>(
        &mut self,
        offset: u32,
        update_fn: F,
    ) -> cu::Result<()> {
        for (_, object) in self.data.iter_mut() {
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

#[derive(Deserialize, Clone, Debug)]
pub struct Object {
    #[serde(rename(serialize = ".text", deserialize = ".text"))]
    pub text_section: Vec<fl::Info>,
}
impl Object {
    pub fn offset(&self) -> u32 {
        // unwrap: validated when loading FileList
        self.text_section.first().unwrap().offset
    }
    /// Format into YAML file list writer
    pub fn fmt_file_list_yml<W: Write>(&self, mut out: W) -> std::fmt::Result {
        let prefix = "  ";
        writeln!(out, "{prefix}'.text':")?;
        for info in &self.text_section {
            info.fmt_file_list_yml(&mut out)?;
        }
        Ok(())
    }
}
