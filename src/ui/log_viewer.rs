use std::collections::VecDeque;

pub struct LogViewer {
    messages: VecDeque<String>,
    max_lines: usize,
}

impl LogViewer {
    pub fn new(max_lines: usize) -> Self {
        LogViewer {
            messages: VecDeque::with_capacity(max_lines),
            max_lines,
        }
    }

    pub fn add(&mut self, msg: String) {
        if self.messages.len() >= self.max_lines {
            self.messages.pop_front();
        }
        self.messages.push_back(msg);
    }

    pub fn draw(&mut self, ui: &imgui::Ui) {
        ui.window("📋 Log Viewer")
            .size([800.0, 400.0], imgui::Condition::FirstUseEver)
            .build(|| {
                for msg in &self.messages {
                    ui.text_wrapped(msg);
                }
            });
    }
}
