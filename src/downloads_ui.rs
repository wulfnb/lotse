use crate::{browser::Browser, downloads::Phase};
pub(crate) struct State {
    pub open: bool,
    pub url: String,
    pub filename: String,
    pub connections: usize,
    pub notice: Option<String>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            open: false,
            url: String::new(),
            filename: String::new(),
            connections: 4,
            notice: None,
        }
    }
}
impl Browser {
    pub fn downloads_window(&mut self, ctx: &egui::Context) {
        if !self.download_ui.open {
            return;
        }
        let mut open = true;
        let mut start = false;
        egui::Window::new("Downloads").open(&mut open).default_width(620.0).default_height(420.0).show(ctx,|ui|{
            ui.label("Download a file");
            ui.add(egui::TextEdit::singleline(&mut self.download_ui.url).hint_text("https://example.com/file.zip").desired_width(f32::INFINITY));
            ui.horizontal(|ui|{ui.label("File name");ui.add(egui::TextEdit::singleline(&mut self.download_ui.filename).hint_text("Automatic").desired_width(250.0));ui.label("Connections");ui.add(egui::DragValue::new(&mut self.download_ui.connections).range(1..=8));start=ui.button("Download").clicked();});
            if let Some(notice)=&self.download_ui.notice{ui.label(notice);}
            ui.weak("Multipart requires byte-range support and a stable file validator. Other servers use a single connection. Completed segments survive pause/resume within this session.");
            ui.horizontal(|ui|{ui.label(format!("Save to: {}",self.downloads.directory.display()));if ui.small_button("Copy folder path").clicked(){ctx.copy_text(self.downloads.directory.display().to_string());}});
            ui.separator();
            egui::ScrollArea::vertical().max_height(420.0).show(ui,|ui|{
                for job in self.downloads.jobs(){
                    let progress=job.progress.lock().unwrap().clone();let bytes=job.bytes();
                    ui.group(|ui|{
                        ui.horizontal(|ui|{ui.strong(&progress.filename);ui.weak(format!("{:?}",progress.phase));});
                        ui.label(egui::RichText::new(&job.url).small()).on_hover_text(&job.url);
                        if let Some(total)=progress.total{ui.add(egui::ProgressBar::new((bytes as f64/total.max(1) as f64).min(1.0) as f32).text(format!("{} / {} · {} connection(s)",size(bytes),size(total),progress.connections)));}else{ui.label(format!("{} · size unknown",size(bytes)));}
                        ui.weak(&progress.message);
                        ui.horizontal(|ui|{
                            match progress.phase {
                                Phase::Connecting|Phase::Downloading=>{ui.weak(format!("~{}/s average",size(job.speed() as u64)));if ui.button("Pause").clicked(){job.pause()}
if ui.button("Cancel").clicked(){job.cancel()}ctx.request_repaint_after(std::time::Duration::from_millis(500));}
                                Phase::Paused=>{if ui.button("Resume").clicked() && let Err(error)=self.downloads.resume(&job){self.download_ui.notice=Some(error);}
if ui.button("Cancel").clicked(){job.cancel()}ctx.request_repaint_after(std::time::Duration::from_millis(500));}
                                Phase::Complete=>{if let Some(path)=&progress.path&&ui.button("Copy saved path").clicked(){ctx.copy_text(path.display().to_string());}}
                                Phase::Failed|Phase::Cancelled=>{if ui.button("Retry…").clicked(){self.download_ui.url=job.url.clone();self.download_ui.filename=progress.filename.clone();}}
                            }
                        });
                    });
                }
            });
            if ui.button("Clear finished entries").clicked(){self.downloads.clear_finished();}
            if std::env::var_os("BROWSER_DOWNLOAD_DIR").is_some(){ui.weak("Docker: use Downloads in the web viewer's top toolbar to save completed files to your computer.");}
        });
        self.download_ui.open = open;
        if start {
            self.begin_download()
        }
    }
}
fn size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    }
}
