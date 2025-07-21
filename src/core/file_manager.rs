use crate::models::MediaFile;

#[derive(Default)]
pub struct FileManager {
    files: Vec<MediaFile>,
    filtered_files: Vec<MediaFile>,
    filter_active: bool,
}

impl FileManager {
    #[must_use]
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

    #[must_use]
    pub fn get_files(&self) -> Vec<MediaFile> {
        if self.filter_active {
            self.filtered_files.clone()
        } else {
            self.files.clone()
        }
    }
    #[must_use]
    pub fn get_file_count(&self) -> usize {
        if self.filter_active {
            self.filtered_files.len()
        } else {
            self.files.len()
        }
    }
}
