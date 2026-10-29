use std::io::Cursor;

use image::{GenericImageView, Pixel};

const MAX_IMAGE_BYTES: usize = 15 * 1024 * 1024;
const MAX_CANDIDATES: usize = 32;
const MAX_DIMENSION: u32 = 8192;
const MIN_FRAME_WIDTH: u32 = 960;
const TARGET_WIDTH: u32 = 640;

fn public_https_url(raw: &str) -> Option<reqwest::Url> {
    tracing::debug!("validating release image URL");
    let url = reqwest::Url::parse(raw).ok()?;
    if url.scheme() != "https" || url.username() != "" || url.password().is_some() || url.port().is_some_and(|port| port != 443) {
        tracing::warn!(scheme = url.scheme(), "release image URL rejected by scheme or authority checks");
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") || host.ends_with(".internal") || host.parse::<std::net::IpAddr>().is_ok() {
        tracing::warn!("release image URL rejected by host checks");
        return None;
    }
    Some(url)
}

async fn download_image(client: &reqwest::Client, url: &reqwest::Url) -> anyhow::Result<Vec<u8>> {
    tracing::debug!(host = url.host_str().unwrap_or_default(), "downloading release description image");
    let mut response = client.get(url.clone()).send().await?;
    if !response.status().is_success() {
        anyhow::bail!("image host returned status {}", response.status());
    }
    if response.content_length().is_some_and(|length| length > MAX_IMAGE_BYTES as u64) {
        anyhow::bail!("release image exceeds byte limit");
    }
    let content_type = response.headers().get(reqwest::header::CONTENT_TYPE).and_then(|value| value.to_str().ok()).unwrap_or_default();
    if !content_type.to_ascii_lowercase().starts_with("image/") {
        anyhow::bail!("release image response has a non-image content type");
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len().saturating_add(chunk.len()) > MAX_IMAGE_BYTES {
            anyhow::bail!("release image exceeds byte limit");
        }
        bytes.extend_from_slice(&chunk);
    }
    tracing::debug!(bytes = bytes.len(), "release description image downloaded");
    Ok(bytes)
}

fn row_is_black(image: &image::DynamicImage, y: u32) -> bool {
    let width = image.width();
    let dark = (0..width).filter(|x| image.get_pixel(*x, y).to_luma().0[0] <= 14).count();
    dark as f32 / width as f32 >= 0.985
}

fn column_is_black(image: &image::DynamicImage, x: u32) -> bool {
    let height = image.height();
    let dark = (0..height).filter(|y| image.get_pixel(x, *y).to_luma().0[0] <= 14).count();
    dark as f32 / height as f32 >= 0.985
}

fn crop_black_bars(image: image::DynamicImage) -> Option<image::DynamicImage> {
    tracing::debug!(width = image.width(), height = image.height(), "checking release image frame dimensions");
    let (width, height) = (image.width(), image.height());
    if width < MIN_FRAME_WIDTH || height < 300 { return None; }
    let max_y_crop = height / 5;
    let mut top = 0;
    let mut bottom = height;
    while top < max_y_crop && top + 1 < bottom && row_is_black(&image, top) { top += 1; }
    while height - bottom < max_y_crop && bottom > top + 1 && row_is_black(&image, bottom - 1) { bottom -= 1; }
    let max_x_crop = width / 5;
    let mut left = 0;
    let mut right = width;
    while left < max_x_crop && left + 1 < right && column_is_black(&image, left) { left += 1; }
    while width - right < max_x_crop && right > left + 1 && column_is_black(&image, right - 1) { right -= 1; }
    let cropped_width = right.saturating_sub(left);
    let cropped_height = bottom.saturating_sub(top);
    if cropped_width < MIN_FRAME_WIDTH || cropped_height == 0 { return None; }
    let ratio = cropped_width as f32 / cropped_height as f32;
    if !(16.0 / 9.0 * 0.95..=16.0 / 9.0 * 1.05).contains(&ratio) {
        tracing::debug!(width = cropped_width, height = cropped_height, ratio, "release image is not a 16:9 frame");
        return None;
    }
    Some(image.crop_imm(left, top, cropped_width, cropped_height))
}

fn decode_frame_jpeg(bytes: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
    tracing::debug!(bytes = bytes.len(), "decoding release description image");
    let reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(160 * 1024 * 1024);
    let mut reader = reader;
    reader.limits(limits);
    let decoded = reader.decode()?;
    let Some(cropped) = crop_black_bars(decoded) else { return Ok(None) };
    let height = ((cropped.height() as u64 * TARGET_WIDTH as u64) / cropped.width() as u64).max(1) as u32;
    let resized = cropped.resize_exact(TARGET_WIDTH, height, image::imageops::FilterType::Lanczos3);
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 88).encode_image(&resized)?;
    tracing::info!(width = TARGET_WIDTH, height, "validated and resized release frame");
    Ok(Some(jpeg))
}

fn sampled_indices(len: usize) -> Vec<usize> {
    tracing::debug!(candidates = len, "sampling release image candidates");
    if len <= MAX_CANDIDATES { return (0..len).collect(); }
    (0..MAX_CANDIDATES).map(|i| i * (len - 1) / (MAX_CANDIDATES - 1)).collect()
}

pub async fn best_frame_jpeg(image_urls: &[String]) -> anyhow::Result<Option<Vec<u8>>> {
    tracing::info!(candidates = image_urls.len(), "searching release page for a usable episode frame");
    if image_urls.is_empty() { return Ok(None); }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("nyaa-stream/0.6.0 thumbnail")
        .build()?;
    let mut valid = Vec::new();
    for index in sampled_indices(image_urls.len()) {
        let Some(url) = public_https_url(&image_urls[index]) else { continue };
        match download_image(&client, &url).await.and_then(|bytes| decode_frame_jpeg(&bytes)) {
            Ok(Some(jpeg)) => valid.push(jpeg),
            Ok(None) => tracing::debug!(index, "release image was not an episode frame"),
            Err(error) => tracing::warn!(index, %error, "release image candidate could not be used"),
        }
    }
    if valid.is_empty() { return Ok(None); }
    let middle = valid.len() / 2;
    tracing::info!(valid = valid.len(), selected = middle, "selected middle valid release frame");
    Ok(Some(valid.swap_remove(middle)))
}
