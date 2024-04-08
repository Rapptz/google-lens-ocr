use std::{env::args_os, io::Cursor, path::PathBuf, sync::OnceLock, time::SystemTime};

use anyhow::{bail, Context};
use arboard::Clipboard;
use image::RgbImage;
use regex::Regex;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum Command {
    Normal(PathBuf),
    Clipboard(PathBuf),
}

impl Command {
    fn new() -> anyhow::Result<Self> {
        let mut args = args_os();
        args.next(); // skip argv[0]
        match args.next() {
            Some(s) if s == "clipboard" => args
                .next()
                .map(|s| Self::Clipboard(PathBuf::from(s)))
                .ok_or(anyhow::anyhow!("missing file to use OCR with")),
            Some(s) => Ok(Self::Normal(PathBuf::from(s))),
            None => anyhow::bail!("missing file to use OCR with"),
        }
    }
}

fn get_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}

fn get_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#">AF_initDataCallback\((\{key: 'ds:1'.*?)\);</script>"#).unwrap()
    })
}

const BOUNDARY: &str = "ZPJQvnUMIqajI5LbS8cc5w";

fn maybe_resize_image(img: RgbImage) -> RgbImage {
    if img.width() * img.height() > 3_000_000 {
        let aspect_ratio = img.width() as f64 / img.height() as f64;
        let nwidth = ((3_000_000f64 * aspect_ratio).sqrt()) as u32;
        let nheight = (nwidth as f64 / aspect_ratio) as u32;
        image::imageops::resize(&img, nwidth, nheight, image::imageops::FilterType::Lanczos3)
    } else {
        img
    }
}

fn create_multipart_form(filename: &str, img: &[u8]) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(img.len() + 500);
    buffer.extend_from_slice(b"--");
    buffer.extend_from_slice(BOUNDARY.as_bytes());
    buffer.extend_from_slice(b"\r\n");
    buffer.extend_from_slice(b"Content-Type: image/png\r\n");
    buffer.extend_from_slice(b"Content-Disposition: form-data; name=\"encoded_image\"; ");
    buffer.extend_from_slice(b"filename=\"");
    buffer.extend_from_slice(filename.as_bytes());
    buffer.extend_from_slice(b"\"\r\n\r\n");
    buffer.extend_from_slice(img);
    buffer.extend_from_slice(b"\r\n--");
    buffer.extend_from_slice(BOUNDARY.as_bytes());
    buffer.extend_from_slice(b"--\r\n");
    buffer
}

fn run_ocr(path: PathBuf) -> anyhow::Result<String> {
    let img = maybe_resize_image(
        image::open(path)
            .context("Could not open image")?
            .into_rgb8(),
    );
    let mut bytes = Vec::new();
    img.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)?;

    let ts = get_timestamp_ms();
    let url = format!("https://lens.google.com/v3/upload?stcs={ts}");
    let body = create_multipart_form(&format!("{ts}.png"), &bytes);
    let resp = ureq::post(&url)
        .set("User-Agent", "Mozilla/5.0 (Linux; Android 13; RMX3771) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.6167.144 Mobile Safari/537.36")
        .set("Cookie", "SOCS=CAESEwgDEgk0ODE3Nzk3MjQaAmVuIAEaBgiA_LyaBg")
        .set("Content-Type", &format!("multipart/form-data; boundary={BOUNDARY}"))
        .send_bytes(&body)?;

    if resp.status() != 200 {
        bail!("Google responded with {}", resp.status());
    }

    let text = resp.into_string()?;

    let capture = get_regex()
        .captures(&text)
        .and_then(|m| m.get(1))
        .context("Could not find object data")?;
    let value = serde_json5::from_str::<serde_json::Value>(capture.as_str())?;
    let data = value
        .pointer("/data/3/4/0")
        .and_then(|s| s.as_array())
        .context("Could not find OCR data")?;

    let Some(data) = data.first().and_then(|s| s.as_array()) else {
        return Ok(String::new());
    };

    let mut buffer = String::new();
    for elem in data {
        let Some(s) = elem.as_str() else {
            continue;
        };
        buffer.push_str(s);
        buffer.push('\n');
    }
    buffer.truncate(buffer.trim_end().len());
    Ok(buffer)
}

fn main() -> anyhow::Result<()> {
    let command = Command::new()?;
    match command {
        Command::Normal(path) => {
            let result = run_ocr(path)?;
            println!("{result}\n");
        }
        Command::Clipboard(path) => {
            let mut clipboard = Clipboard::new().context("Could not open clipboard")?;
            let result = run_ocr(path)?;
            clipboard
                .set_text(result)
                .context("Could not set clipboard contents")?;
        }
    };
    Ok(())
}
