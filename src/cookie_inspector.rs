//! Engine cookies stay in native state and are never injected into the page.
use crossbeam_channel::Receiver;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct Row {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub expires: String,
    pub same_site: String,
    pub http_only: bool,
    pub secure: bool,
}

#[derive(Default)]
pub struct State {
    pub rows: Vec<Row>,
    pub error: Option<String>,
    pub loaded: bool,
    pub filter: String,
    pending: Option<(Receiver<Vec<Row>>, Instant)>,
}

impl State {
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn refresh(&mut self, view: &servo::WebView, servo: &servo::Servo) {
        self.rows.clear();
        self.error = None;
        self.loaded = false;
        self.pending = None;
        let Some(url) = view
            .url()
            .filter(|u| matches!(u.scheme(), "http" | "https"))
        else {
            self.error = Some("Cookie inspection requires an HTTP or HTTPS page.".into());
            return;
        };
        let host = url.host_str().unwrap_or_default().to_owned();
        let (sender, receiver) = crossbeam_channel::bounded(1);
        servo.site_data_manager().cookies_for_url_async(
            url,
            servo::CookieSource::HTTP,
            move |cookies| {
                let mut rows: Vec<_> = cookies
                    .into_iter()
                    .map(|cookie| Row {
                        name: cookie.name().into(),
                        value: cookie.value().into(),
                        domain: cookie.domain().unwrap_or(&host).into(),
                        path: cookie.path().unwrap_or("/").into(),
                        expires: cookie
                            .expires_datetime()
                            .map(|date| date.to_string())
                            .unwrap_or_else(|| "Session".into()),
                        same_site: cookie
                            .same_site()
                            .map(|value| value.to_string())
                            .unwrap_or_else(|| "Unspecified".into()),
                        http_only: cookie.http_only().unwrap_or(false),
                        secure: cookie.secure().unwrap_or(false),
                    })
                    .collect();
                rows.sort_by(|a, b| {
                    (&a.domain, &a.path, &a.name).cmp(&(&b.domain, &b.path, &b.name))
                });
                let _ = sender.send(rows);
            },
        );
        self.pending = Some((receiver, Instant::now() + Duration::from_secs(10)));
    }

    pub fn poll(&mut self) {
        let Some((receiver, deadline)) = &self.pending else {
            return;
        };
        match receiver.try_recv() {
            Ok(rows) => {
                self.rows = rows;
                self.loaded = true;
                self.pending = None;
            }
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                self.error =
                    Some("The engine cookie request was cancelled. Refresh to retry.".into());
                self.pending = None;
            }
            Err(_) if Instant::now() >= *deadline => {
                self.error = Some("The engine cookie request timed out. Refresh to retry.".into());
                self.pending = None;
            }
            Err(_) => {}
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.weak("Cookies matching the active page URL, including HttpOnly cookies. Values are read from Servo and remain in native developer tools.");
        if self.pending() {
            ui.spinner();
        }
        if let Some(error) = &self.error {
            ui.label(error);
        }
        ui.horizontal(|ui| {
            ui.label("Filter cookies");
            ui.text_edit_singleline(&mut self.filter);
        });
        let filter = self.filter.to_lowercase();
        let rows: Vec<_> = self
            .rows
            .iter()
            .filter(|row| {
                [&row.name, &row.domain, &row.path]
                    .iter()
                    .any(|value| value.to_lowercase().contains(&filter))
            })
            .collect();
        ui.label(format!("{} of {} cookies", rows.len(), self.rows.len()));
        if self.loaded && rows.is_empty() {
            ui.weak("No matching cookies.");
        }
        egui::Grid::new("engine-cookie-rows")
            .striped(true)
            .show(ui, |ui| {
                for title in [
                    "Name", "Value", "Domain", "Path", "Expires", "SameSite", "HttpOnly", "Secure",
                ] {
                    ui.strong(title);
                }
                ui.end_row();
                for row in rows {
                    for text in [
                        &row.name,
                        &row.value,
                        &row.domain,
                        &row.path,
                        &row.expires,
                        &row.same_site,
                    ] {
                        ui.add(egui::Label::new(text).selectable(true));
                    }
                    ui.label(if row.http_only { "Yes" } else { "No" });
                    ui.label(if row.secure { "Yes" } else { "No" });
                    ui.end_row();
                }
            });
        ui.weak("Cookie editing and IndexedDB inspection are not available yet.");
    }
}

#[cfg(test)]
#[path = "cookie_inspector_tests.rs"]
mod tests;
