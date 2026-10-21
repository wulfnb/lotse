//! Desktop cookie adapter and download coordination; no widget rendering.
use crate::{browser::Browser, downloads};
impl Browser {
    pub fn offer_download(&mut self, url: &str, filename: &str) {
        self.download_ui.open = true;
        self.download_ui.url = url.into();
        self.download_ui.filename = filename.into();
        self.download_ui.notice = None;
    }
    pub fn begin_download(&mut self) {
        let result = (|| {
            let url = downloads::validate_url(&self.download_ui.url)?;
            let mut cookies = String::from("# Netscape HTTP Cookie File\n");
            for cookie in self
                .servo
                .site_data_manager()
                .cookies_for_url(url.clone(), servo::CookieSource::HTTP)
            {
                let name = cookie.name();
                let value = cookie.value();
                let path = cookie.path().unwrap_or("/");
                if [name, value, path]
                    .iter()
                    .any(|s| s.chars().any(char::is_control))
                {
                    continue;
                }
                cookies.push_str(&format!(
                    "{}\tFALSE\t{}\t{}\t0\t{}\t{}\n",
                    url.host_str().unwrap(),
                    path,
                    if cookie.secure().unwrap_or(false) {
                        "TRUE"
                    } else {
                        "FALSE"
                    },
                    name,
                    value
                ));
            }
            self.start_download(
                url.as_str(),
                &self.download_ui.filename.clone(),
                self.download_ui.connections,
                cookies,
            )
        })();
        self.download_ui.notice = Some(match result {
            Ok(()) => "Download started.".into(),
            Err(error) => error,
        });
    }
}
