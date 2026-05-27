use quad_snd::AudioContext;
use std::path::Path;

pub struct AudioPlayer {
    ctx: AudioContext,
}

impl AudioPlayer {
    pub fn new() -> Self {
        let ctx = AudioContext::new();
        AudioPlayer { ctx }
    }

    pub fn play_file(&self, path: &Path) -> Result<(), String> {
        let bytes = std::fs::read(path)
            .map_err(|e| format!("فشل قراءة الملف الصوتي {}: {}", path.display(), e))?;
        let sound = quad_snd::Sound::load(&self.ctx, &bytes);
        sound.play(&self.ctx, Default::default()); // يرجع Playback، لا Result
        log::info!("🔊 تم تشغيل الصوت: {}", path.display());
        Ok(())
    }

    pub fn play_from_memory(&self, data: &[u8]) -> Result<(), String> {
        let sound = quad_snd::Sound::load(&self.ctx, data);
        sound.play(&self.ctx, Default::default());
        log::info!("🔊 تم تشغيل صوت من الذاكرة ({} بايت)", data.len());
        Ok(())
    }
}

pub fn play_all_assets(audio: &AudioPlayer) {
    let sounds_dir = "assets/sounds";
    if let Ok(entries) = std::fs::read_dir(sounds_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "ogg") {
                log::info!("تشغيل: {}", path.display());
                if let Err(e) = audio.play_file(&path) {
                    log::error!("خطأ في تشغيل {}: {}", path.display(), e);
                }
            }
        }
    } else {
        log::warn!("مجلد الأصوات غير موجود: {}", sounds_dir);
    }
}
