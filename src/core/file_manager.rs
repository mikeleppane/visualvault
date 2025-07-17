use crate::models::MediaFile;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use std::collections::HashMap;

pub struct FileManager {
    files: Vec<MediaFile>,
    filtered_files: Vec<MediaFile>,
    filter_active: bool,
}

impl FileManager {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            filtered_files: Vec::new(),
            filter_active: false,
        }
    }

    pub fn set_files(&mut self, files: Vec<MediaFile>) {
        self.files = files;
        self.filtered_files = self.files.clone();
        self.filter_active = false;
    }

    pub fn get_files(&self) -> Vec<MediaFile> {
        if self.filter_active {
            self.filtered_files.clone()
        } else {
            self.files.clone()
        }
    }

    pub fn get_file_count(&self) -> usize {
        if self.filter_active {
            self.filtered_files.len()
        } else {
            self.files.len()
        }
    }

    pub fn filter_files(&mut self, query: &str) {
        if query.is_empty() {
            self.filter_active = false;
            self.filtered_files = self.files.clone();
            return;
        }

        self.filter_active = true;
        let matcher = SkimMatcherV2::default();

        self.filtered_files = self
            .files
            .iter()
            .filter(|file| {
                // Check filename
                if matcher.fuzzy_match(&file.name, query).is_some() {
                    return true;
                }

                // Check file type
                let type_str = format!("{:?}", file.file_type).to_lowercase();
                if type_str.contains(&query.to_lowercase()) {
                    return true;
                }

                // Check extension
                if file.extension.contains(&query.to_lowercase()) {
                    return true;
                }

                false
            })
            .cloned()
            .collect();
    }

    pub fn get_duplicates(&self) -> HashMap<String, Vec<MediaFile>> {
        let mut hash_map: HashMap<String, Vec<MediaFile>> = HashMap::new();

        for file in &self.files {
            if let Some(hash) = &file.hash {
                hash_map.entry(hash.clone()).or_default().push(file.clone());
            }
        }

        hash_map.retain(|_, files| files.len() > 1);
        hash_map
    }

    pub fn remove_file(&mut self, path: &std::path::Path) {
        self.files.retain(|f| f.path != path);
        if self.filter_active {
            self.filtered_files.retain(|f| f.path != path);
        }
    }
}
