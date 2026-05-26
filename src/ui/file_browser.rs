pub struct FileBrowser {
    current_path: String,
    entries: Vec<String>,
}

impl FileBrowser {
    pub fn new(start_path: &str) -> Self {
        let mut fb = FileBrowser {
            current_path: start_path.to_string(),
            entries: Vec::new(),
        };
        fb.refresh();
        fb
    }

    fn refresh(&mut self) {
        self.entries.clear();
        self.entries.push("📁 ..".to_string());
        if let Ok(dir) = std::fs::read_dir(&self.current_path) {
            for entry in dir.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if entry.path().is_dir() {
                    self.entries.push(format!("📁 {}", name));
                } else if name.ends_with(".so") || name.ends_with(".apk") {
                    self.entries.push(format!("🎮 {}", name));
                } else {
                    self.entries.push(format!("📄 {}", name));
                }
            }
        }
    }

    pub fn draw(&mut self, ui: &imgui::Ui, on_select: &mut dyn FnMut(String)) {
        ui.window("📂 File Browser")
            .size([600.0, 700.0], imgui::Condition::FirstUseEver)
            .build(|| {
                ui.text(format!("Path: {}", self.current_path));
                ui.separator();

                let mut selected_idx: Option<usize> = None;
                for (i, entry) in self.entries.iter().enumerate() {
                    if ui.selectable_config(entry).build() {
                        selected_idx = Some(i);
                    }
                }

                if let Some(idx) = selected_idx {
                    let entry = &self.entries[idx];
                    if entry == "📁 .." {
                        if let Some(parent) = std::path::Path::new(&self.current_path).parent() {
                            self.current_path = parent.to_string_lossy().to_string();
                            self.refresh();
                        }
                    } else if entry.starts_with("📁 ") {
                        let dir_name = &entry[6..];
                        let new_path = format!("{}/{}", self.current_path, dir_name);
                        self.current_path = new_path;
                        self.refresh();
                    } else if entry.starts_with("🎮 ") || entry.starts_with("📄 ") {
                        let file_name = entry[6..].to_string();
                        let full_path = format!("{}/{}", self.current_path, file_name);
                        on_select(full_path);
                    }
                }
            });
    }
}
