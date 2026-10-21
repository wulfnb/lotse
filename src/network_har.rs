//! Bounded HAR 1.2 interchange. Validate an entire import before touching live rows.
use super::*;
use base64::Engine;
pub const FILE_LIMIT: usize = 16 * 1024 * 1024;
fn content(body: &Body, mime: &str) -> Value {
    let mut value = json!({"size":body.size,"mimeType":mime});
    if let Some(text) = body.text() {
        value["text"] = json!(text)
    } else {
        value["text"] = json!(base64::engine::general_purpose::STANDARD.encode(&body.bytes));
        value["encoding"] = json!("base64");
    }
    if body.truncated() {
        value["comment"] = json!("Preview truncated at 64 KiB");
    }
    value
}
pub(super) fn export<'a>(rows: impl Iterator<Item = &'a Request>) -> Value {
    let entries:Vec<_>=rows.map(|r|{
        let mut post=content(&r.request_body,r.request_headers.iter().find(|(k,_)|k.eq_ignore_ascii_case("content-type")).map(|(_,v)|v.as_str()).unwrap_or(""));
        // bodySize belongs on request, not postData. encoding is a documented extension for binary request previews.
        post.as_object_mut().unwrap().remove("size");
        json!({"startedDateTime":iso_date(r.started_ms),"time":r.duration_ms(),
            "request":{"method":r.method,"url":r.url,"httpVersion":"","cookies":[],"headers":har_headers(&r.request_headers),"queryString":url::Url::parse(&r.url).ok().map(|u|u.query_pairs().map(|(name,value)|json!({"name":name,"value":value})).collect::<Vec<_>>()).unwrap_or_default(),"headersSize":-1,"bodySize":r.request_body.size,"postData":post},
            "response":{"status":r.status.unwrap_or(0),"statusText":r.reason,"httpVersion":"","cookies":[],"headers":har_headers(&r.response_headers),"content":content(&r.response_body,r.header("content-type").unwrap_or("")),"redirectURL":r.header("location").unwrap_or(""),"headersSize":-1,"bodySize":r.transfer().and_then(|n|i64::try_from(n).ok()).unwrap_or(-1)},
            "cache":{},"timings":{"blocked":-1,"dns":-1,"connect":r.connect_ms,"send":r.send_ms,"wait":-1,"receive":-1,"ssl":-1},
            "_fromCache":r.cached,"_resourceType":r.kind,"_initiator":{"type":r.initiator},
            "comment":"Total is observed event time or imported HAR time. Unknown phase timings are -1. Bodies are bounded previews; wire transfer size is unknown unless reported."})
    }).collect();
    json!({"log":{"version":"1.2","creator":{"name":crate::brand::APP_NAME,"version":env!("CARGO_PKG_VERSION")},"entries":entries}})
}
fn headers(value: &Value) -> Result<Vec<(String, String)>, String> {
    let Some(items) = value.as_array() else {
        if value.is_null() {
            return Ok(vec![]);
        }
        return Err("HAR headers must be an array".into());
    };
    if items.len() > 200 {
        return Err("HAR contains more than 200 headers in a request or response".into());
    }
    items
        .iter()
        .map(|item| {
            let name = item["name"]
                .as_str()
                .ok_or("HAR header name must be a string")?;
            let value = item["value"]
                .as_str()
                .ok_or("HAR header value must be a string")?;
            if name.len() > 1024 || value.len() > 8192 {
                return Err("HAR header is too large".into());
            }
            Ok((name.to_string(), value.to_string()))
        })
        .collect()
}
fn body(value: &Value, size: Option<u64>) -> Result<Body, String> {
    let text = value["text"].as_str().unwrap_or("");
    let data = if value["encoding"] == "base64" {
        base64::engine::general_purpose::STANDARD
            .decode(text)
            .map_err(|_| "HAR contains invalid base64 content")?
    } else {
        text.as_bytes().to_vec()
    };
    let size = size
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(data.len())
        .max(data.len());
    Ok(Body {
        bytes: data[..data.len().min(BODY_LIMIT)].to_vec(),
        size,
    })
}
fn bounded(value: &Value, key: &str, max: usize) -> Result<String, String> {
    let text = value[key]
        .as_str()
        .ok_or_else(|| format!("HAR {key} must be a string"))?;
    if text.len() > max {
        return Err(format!("HAR {key} is too long"));
    }
    Ok(text.into())
}
pub(super) fn import(recorder: &mut Recorder, owner: &str, bytes: &[u8]) -> Result<usize, String> {
    if bytes.len() > FILE_LIMIT {
        return Err("HAR files are limited to 16 MiB".into());
    }
    let value: Value =
        serde_json::from_slice(bytes).map_err(|e| format!("Invalid HAR JSON: {e}"))?;
    let entries = value["log"]["entries"]
        .as_array()
        .ok_or("HAR must contain log.entries")?;
    if entries.len() > ROW_LIMIT {
        return Err(format!("HAR import is limited to {ROW_LIMIT} requests"));
    }
    let now = Instant::now();
    let mut pending = Vec::new();
    for (index, e) in entries.iter().enumerate() {
        let url = bounded(&e["request"], "url", 65536)?;
        url::Url::parse(&url).map_err(|_| "HAR request URL is invalid")?;
        let method = bounded(&e["request"], "method", 32)?;
        http::Method::from_bytes(method.as_bytes()).map_err(|_| "HAR method is invalid")?;
        let duration = e["time"]
            .as_f64()
            .filter(|t| t.is_finite() && *t >= 0.0)
            .ok_or("HAR time must be a nonnegative number")?;
        let date = e["startedDateTime"]
            .as_str()
            .ok_or("HAR startedDateTime is missing")?;
        let started_ms = chrono::DateTime::parse_from_rfc3339(date)
            .map_err(|_| "HAR startedDateTime is invalid")?
            .timestamp_millis();
        if started_ms < 0 {
            return Err("HAR timestamps before 1970 are unsupported".into());
        }
        let response_headers = headers(&e["response"]["headers"])?;
        let content = &e["response"]["content"];
        let mime = content["mimeType"].as_str().unwrap_or("");
        let resource = e["_resourceType"].as_str().unwrap_or("");
        let kind = match resource.to_ascii_lowercase().as_str() {
            "html" | "document" => "HTML",
            "script" | "js" => "JS",
            "css" | "stylesheet" => "CSS",
            "xhr" => "XHR",
            "fetch" => "Fetch",
            "img" | "image" => "Img",
            "font" => "Font",
            "media" => "Media",
            "websocket" | "ws" => "WS",
            _ if mime.contains("html") => "HTML",
            _ if mime.contains("css") => "CSS",
            _ if mime.contains("javascript") => "JS",
            _ if mime.starts_with("image/") => "Img",
            _ if mime.starts_with("font/") => "Font",
            _ if mime.starts_with("audio/") || mime.starts_with("video/") => "Media",
            _ => "Other",
        };
        let status = e["response"]["status"]
            .as_u64()
            .and_then(|n| u16::try_from(n).ok())
            .filter(|n| *n <= 599)
            .ok_or("HAR response status must be between 0 and 599")?;
        let reason = e["response"]["statusText"].as_str().unwrap_or("");
        if reason.len() > 8192 {
            return Err("HAR status text is too long".into());
        }
        let initiator = e["_initiator"]["type"]
            .as_str()
            .or_else(|| e["_cause"]["type"].as_str())
            .unwrap_or("");
        if initiator.len() > 1024 {
            return Err("HAR initiator is too long".into());
        }
        pending.push(Request {
            uid: 0,
            id: format!("har-{index}"),
            owner: owner.into(),
            url,
            method,
            kind: kind.into(),
            request_headers: headers(&e["request"]["headers"])?,
            response_headers,
            request_body: body(&e["request"]["postData"], e["request"]["bodySize"].as_u64())?,
            response_body: body(content, content["size"].as_u64())?,
            status: Some(status),
            reason: reason.into(),
            started: now,
            started_ms: started_ms as u64,
            completed: Some(now),
            cached: e["_fromCache"].as_bool().unwrap_or(false),
            connect_ms: e["timings"]["connect"]
                .as_f64()
                .filter(|v| v.is_finite())
                .unwrap_or(-1.0),
            send_ms: e["timings"]["send"]
                .as_f64()
                .filter(|v| v.is_finite())
                .unwrap_or(-1.0),
            security: String::new(),
            imported: true,
            recorded_duration: Some(duration),
            transferred: e["_transferSize"]
                .as_u64()
                .or_else(|| e["response"]["bodySize"].as_u64()),
            initiator: initiator.into(),
        });
    }
    let count = pending.len();
    for mut row in pending {
        recorder.next += 1;
        row.uid = recorder.next;
        row.id = format!("har-{}", row.uid);
        recorder.rows.push_back(row);
    }
    while recorder.rows.len() > ROW_LIMIT {
        recorder.rows.pop_front();
    }
    recorder.selected = None;
    Ok(count)
}
#[cfg(test)]
#[path = "network_har_tests.rs"]
mod tests;
