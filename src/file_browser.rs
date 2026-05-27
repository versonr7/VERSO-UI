use dear_file_browser::{FileBrowser, Filter};

/// إنشاء متصفح ملفات مخصص لاختيار APK/SO
pub fn create_browser() -> FileBrowser {
    let mut browser = FileBrowser::new();
    
    // إعداد المسار الافتراضي
    browser.set_path("/storage/emulated/0/Download");
    
    // تصفية الملفات (APK و SO فقط)
    browser.set_filter(Filter::new().extensions(&["apk", "so"]));
    
    // تخصيص المظهر
    browser.set_title("Select Game File");
    
    browser
}
