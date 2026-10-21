//! Page Performance API timeline. These are timings, not sampled CPU stacks.
use crate::{
    browser::Browser,
    toolbox::{Tool, pretty},
};
use egui::{RichText, Ui};
use serde_json::Value;

#[derive(Default)]
pub struct State {
    pub query: String,
    pub selected: Option<usize>,
    pub range_start: f64,
    pub range_end: f64,
    pub summary: bool,
    pub zoomed: bool,
}

pub const START: &str = r#"
if(globalThis.__servoPerfRecording?.running)throw Error('A recording is already running');
const capture={running:true,start:performance.now(),end:null,frames:[],entries:[],truncated:false};
globalThis.__servoPerfRecording=capture;
if(typeof PerformanceObserver!=='undefined'){
 try{capture.observer=new PerformanceObserver(list=>{for(const e of list.getEntries()){if(capture.entries.length<2000)capture.entries.push(e.toJSON());else capture.truncated=true;}});
 capture.observer.observe({entryTypes:['resource','mark','measure','paint','longtask']});}catch(e){capture.observerError=String(e);}
}
function frame(time){if(!capture.running)return;if(capture.frames.length<3600)capture.frames.push(time);else capture.truncated=true;if(time-capture.start>=60000){capture.running=false;capture.end=time;capture.observer?.disconnect();return;}capture.raf=requestAnimationFrame(frame);}
capture.raf=requestAnimationFrame(frame);
"#;
pub const STOP: &str = r#"
const capture=globalThis.__servoPerfRecording;
if(capture){capture.running=false;capture.end=performance.now();cancelAnimationFrame(capture.raf);if(capture.observer){for(const e of capture.observer.takeRecords()){if(capture.entries.length<2000)capture.entries.push(e.toJSON());else capture.truncated=true;}capture.observer.disconnect();}}
"#;

pub fn entries(data: &Value) -> Vec<Value> {
    let mut entries = Vec::new();
    for (key, kind) in [
        ("navigation", "navigation"),
        ("resources", "resource"),
        ("marks", "mark"),
        ("measures", "measure"),
        ("paint", "paint"),
        ("longtasks", "longtask"),
    ] {
        if let Some(rows) = data[key].as_array() {
            for row in rows {
                let mut entry = row.clone();
                entry["kind"] = kind.into();
                entries.push(entry);
            }
        }
    }
    entries.sort_by(|a, b| number(a, "start").total_cmp(&number(b, "start")));
    entries
}
fn number(value: &Value, key: &str) -> f64 {
    value[key]
        .as_f64()
        .filter(|n| n.is_finite())
        .unwrap_or(0.0)
        .max(0.0)
}

impl Browser {
    pub fn performance_ui(
        &mut self,
        ui: &mut Ui,
        view: &servo::WebView,
        owner: &str,
        data: &Value,
    ) {
        let colors = crate::theme::palette(ui.ctx());
        let running = data["recording"]["running"] == true;
        let pending = self.tools.pages[owner]
            .snapshots
            .get(&Tool::Performance)
            .is_some_and(|s| s.pending);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !pending,
                    egui::Button::new(if running {
                        "Stop recording"
                    } else {
                        "Start recording"
                    }),
                )
                .clicked()
            {
                let prefix = if running { STOP } else { START };
                self.tools.page(view).performance.selected = None;
                self.tools.page(view).performance.range_end = 0.0;
                self.tools.page(view).performance.zoomed = false;
                self.tools.run(
                    view,
                    Tool::Performance,
                    format!("{prefix}{}", crate::probes::PERFORMANCE),
                );
            }
            if ui.button("Copy timing JSON").clicked() {
                ui.ctx().copy_text(pretty(data));
            }
            let state = &mut self.tools.page(view).performance;
            ui.selectable_value(&mut state.summary, false, "Waterfall");
            ui.selectable_value(&mut state.summary, true, "Summary");
            ui.add(
                egui::TextEdit::singleline(&mut state.query)
                    .hint_text("Filter entries")
                    .desired_width(180.0),
            );
        });
        ui.weak("Page timings and animation-frame intervals. CPU stacks, flame graphs and engine-thread sampling are unavailable.");
        if running {
            ui.colored_label(
                colors.amber,
                "Recording · press Refresh for progress · automatically stops after 60 seconds",
            );
        }
        if data["recording"]["truncated"] == true {
            ui.colored_label(
                colors.amber,
                "Capture limit reached; some entries or frames were omitted.",
            );
        }
        if let Some(error) = data["recording"]["observerError"].as_str() {
            ui.weak(error);
        }
        let rows = entries(data);
        let capture_start = number(&data["recording"], "start");
        let duration = if data["recording"].is_object() {
            number(&data["recording"], "end") - capture_start
        } else {
            number(data, "now")
        }
        .max(1.0);
        let state = &mut self.tools.page(view).performance;
        if !state.zoomed || state.range_end <= 0.0 {
            state.range_end = duration;
        }
        state.range_end = state.range_end.min(duration).max(0.001);
        state.range_start = state.range_start.min(state.range_end);
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("{:.1} ms · {} entries", duration, rows.len()));
            ui.label("Range (ms)");
            let start_changed = ui.add(
                egui::DragValue::new(&mut state.range_start)
                    .range(0.0..=state.range_end)
                    .speed(1.0),
            );
            let end_changed = ui.add(
                egui::DragValue::new(&mut state.range_end)
                    .range(state.range_start.max(0.001)..=duration)
                    .speed(1.0),
            );
            if start_changed.changed() || end_changed.changed() {
                state.zoomed = true;
            }
            if ui.button("Reset zoom").clicked() {
                state.zoomed = false;
                state.range_start = 0.0;
                state.range_end = duration;
            }
        });
        let start = capture_start + state.range_start;
        let end = capture_start + state.range_end;
        let query = state.query.to_lowercase();
        let visible: Vec<_> = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                number(r, "start") <= end
                    && number(r, "start") + number(r, "duration") >= start
                    && r["name"]
                        .as_str()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query)
            })
            .collect();
        let selected = state.selected.and_then(|i| rows.get(i)).cloned();
        if let Some(entry) = selected {
            egui::Panel::right("performance-details")
                .default_size(280.0)
                .min_size(120.0)
                .resizable(true)
                .show(ui, |ui| {
                    if ui.button("Close details").clicked() {
                        self.tools.page(view).performance.selected = None;
                    }
                    egui::ScrollArea::both()
                        .id_salt("performance-entry")
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(RichText::new(pretty(&entry)).monospace())
                                    .selectable(true),
                            );
                        });
                });
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui,|ui|{
            egui::ScrollArea::both().id_salt("performance-timeline").auto_shrink([false,false]).show(ui,|ui|{
                let state=&mut self.tools.page(view).performance;
                if state.summary {
                    ui.strong("Timing summary for selected range");
                    for kind in ["navigation","resource","mark","measure","paint"] {
                        let matching:Vec<_>=visible.iter().filter(|(_,r)|r["kind"]==kind).collect();
                        let total:f64=matching.iter().map(|(_,r)|number(r,"duration")).sum();
                        ui.label(format!("{kind}: {} entries · {total:.2} ms total (overlapping durations)",matching.len()));
                    }
                    if let Some(frames)=data["recording"]["frames"].as_array(){
                        let values:Vec<_>=frames.iter().filter_map(Value::as_f64).collect();
                        let intervals:Vec<_>=values.windows(2).filter(|w|w[1]>=start&&w[1]<=end).map(|w|w[1]-w[0]).collect();
                        let max=intervals.iter().copied().fold(0.0,f64::max);
                        let mean=intervals.iter().sum::<f64>()/intervals.len().max(1) as f64;
                        ui.label(format!("Animation frames: {} intervals · mean {mean:.2} ms · longest {max:.2} ms",intervals.len()));
                        ui.weak("Animation-frame intervals reflect rendering callbacks, not CPU utilization.");
                    }
                } else {
                    let width=ui.available_width().max(300.0);
                    let label_width=(width*0.42).clamp(100.0,320.0);
                    ui.label(format!("Entry / category                          {:.1} -> {:.1} ms",state.range_start,state.range_end));
                    for (index,row) in &visible {
                        let (rect,response)=ui.allocate_exact_size(egui::vec2(width,26.0),egui::Sense::click());
                        if state.selected==Some(*index){ui.painter().rect_filled(rect,0,colors.selected);}
                        let label=egui::Rect::from_min_max(rect.min,egui::pos2(rect.left()+label_width,rect.bottom()));
                        ui.painter().with_clip_rect(label.intersect(ui.clip_rect())).text(label.left_center(),egui::Align2::LEFT_CENTER,format!("{} · {}",row["kind"].as_str().unwrap_or(""),row["name"].as_str().unwrap_or("")),egui::FontId::proportional(12.0),colors.text);
                        let x=rect.left()+label_width+8.0;
                        let timeline=(width-label_width-16.0).max(1.0);
                        let span=(end-start).max(0.001);
                        let left=x+timeline*((number(row,"start").max(start)-start)/span) as f32;
                        let right=x+timeline*((number(row,"start")+number(row,"duration")).min(end)-start).max(0.0) as f32/span as f32;
                        let bar=egui::Rect::from_min_size(egui::pos2(left,rect.top()+6.0),egui::vec2((right-left).max(2.0).min((x+timeline-left).max(2.0)),14.0));
                        ui.painter().rect_filled(bar,2,if row["kind"]=="resource"{colors.accent}else{colors.amber});
                        if response.on_hover_text(format!("{}\nStart: {:.2} ms · Duration: {:.2} ms",row["name"].as_str().unwrap_or(""),number(row,"start"),number(row,"duration"))).clicked(){state.selected=Some(*index);}
                    }
                    if visible.is_empty(){ui.weak("No timing entries in this range. Record page interactions or reset the range.");}
                }
            });
        });
    }
}
