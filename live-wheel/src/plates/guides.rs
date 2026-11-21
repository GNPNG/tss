use super::*;

pub(super) fn write_guides(dir: &Path, format: DigitalFormat) -> Result<(), String> {
    use std::fmt::Write as _;
    let [w, h] = format.size();
    let mut guide = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 {w} {h}" width="{w}" height="{h}"><title>Composition guides</title><image xlink:href="projection.png" width="{w}" height="{h}"/><g fill="none" stroke="white" stroke-width="5" stroke-dasharray="20 12">"#
    );
    match format.layout {
        Layout::Visualizer => if let Some(y)=format.horizon_y { write!(guide,r#"<path d="M0 {y}H{w}"><title>Horizon at y={y}</title></path>"#).unwrap(); },
        Layout::DspCover => {},
        Layout::Vertical => guide.push_str(r#"<rect x="0" y="576" width="2160" height="499.2"><title>Sigil and band title, 15–28%</title></rect><rect x="0" y="3072" width="2160" height="768" fill="black" fill-opacity=".25"><title>Lower 20% reserved for platform UI</title></rect>"#),
        Layout::Panorama => for n in 1..5 {write!(guide,r#"<path d="M{} 0v{h}"/>"#,2160*n).unwrap();},
    }
    guide.push_str("</g></svg>\n");
    let mut file = File::create_new(dir.join("guides.svg")).map_err(|e| e.to_string())?;
    file.write_all(guide.as_bytes()).map_err(|e| e.to_string())
}
