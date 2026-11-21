use super::discrete_renderer;
use crate::warp_diagnostic::read_rgba16;
use crate::{FilmOil, LiveWheel, MaterialMotion};
use flate2::{write::GzEncoder, Compression};
use serde_json::json;
use std::{fs, io::Write, path::PathBuf};

const SIZE: [u32; 2] = [640, 360];
const FRAMES: u32 = 96;
const RATE: u32 = 24;

fn select(renderer: &mut LiveWheel, rear: FilmOil, front: FilmOil) -> Result<(), String> {
    renderer.set_material_motion(MaterialMotion { rear, front })
}

fn fields(renderer: &mut LiveWheel, t: f64) -> Result<Vec<[u16; 4]>, String> {
    renderer.render_film_times_offscreen(t, t, t)?;
    Ok(read_rgba16(
        renderer,
        &renderer.interpolated,
        renderer.field_size(),
    ))
}

fn changed(a: &[[u16; 4]], b: &[[u16; 4]], channel: usize) -> usize {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .filter(|(x, y)| x[channel] != y[channel])
        .count()
}

#[test]
#[ignore = "requires a reserved discrete GPU and a new capture directory"]
fn compare_projected_oils() -> Result<(), Box<dyn std::error::Error>> {
    let dir =
        PathBuf::from(std::env::var_os("TSS_OIL_CAPTURE_DIR").ok_or("Missing capture directory")?);
    fs::create_dir(&dir)?;
    let (mut r, adapter) = discrete_renderer(SIZE)?;
    r.set_preview_width(SIZE[0])?;
    r.set_footprint_sampling(true);
    r.set_wheel("violet-blue")?;
    r.set_focus(Some(0.9))?;

    select(&mut r, FilmOil::Pdms20, FilmOil::Pdms20)?;
    let light_start = fields(&mut r, 0.)?;
    let light = fields(&mut r, 2.)?;
    select(&mut r, FilmOil::Pdms20, FilmOil::Pdms1000)?;
    let front = fields(&mut r, 2.)?;
    select(&mut r, FilmOil::Pdms1000, FilmOil::Pdms1000)?;
    let heavy_start = fields(&mut r, 0.)?;
    let both = fields(&mut r, 2.)?;
    let initial = [
        changed(&light_start, &heavy_start, 0),
        changed(&light_start, &heavy_start, 1),
    ];
    let front_edit = [changed(&light, &front, 0), changed(&light, &front, 1)];
    let rear_edit = [changed(&front, &both, 0), changed(&front, &both, 1)];
    assert_eq!(
        initial,
        [0, 0],
        "Switching oil grade changed the starting contour"
    );
    assert!(
        front_edit[0] == 0 && front_edit[1] > 0,
        "Front edit: {front_edit:?}"
    );
    assert!(
        rear_edit[0] > 0 && rear_edit[1] == 0,
        "Rear edit: {rear_edit:?}"
    );
    assert!(
        light_start.iter().any(|p| p[0] != p[1]),
        "The two physical slides need different contours"
    );

    let mut movie_matches = 0;
    for (oil, name) in [
        (FilmOil::Pdms20, "front-20"),
        (FilmOil::Pdms1000, "front-1000"),
    ] {
        select(&mut r, FilmOil::Wheel, oil)?;
        let handle = fs::File::create_new(dir.join(format!("{name}.rgba.gz")))?;
        let mut film = GzEncoder::new(std::io::BufWriter::new(handle), Compression::fast());
        for frame in 0..FRAMES {
            let t = f64::from(frame) / f64::from(RATE);
            r.render_film_times_offscreen(t, t, t)?;
            let (w, h, rgba) = r.capture_rgba8()?;
            assert_eq!([w, h], SIZE);
            film.write_all(&rgba)?;
            if frame == RATE * 2 {
                let mut movie = pollster::block_on(r.begin_movie(SIZE, true))?;
                movie.frame_films(t, t, t, |pixels| {
                    if pixels == rgba {
                        Ok(())
                    } else {
                        Err("Movie and preview differ for the same oil grade".into())
                    }
                })?;
                movie_matches += 1;
            }
        }
        film.finish()?.flush()?;
    }
    let report = json!({
        "adapter":adapter.name,"backend":format!("{:?}",adapter.backend),
        "size":SIZE,"fps":RATE,"framesPerGrade":FRAMES,
        "wheel":"violet-blue","focus":0.9,"footprintSampling":true,
        "movieRear":"wheel","movieFronts":["pdms-20","pdms-1000"],
        "fieldSize":r.field_size(),"fieldChannels":["rear","front"],
        "initialChangedSamples":initial,"comparisonTimeSeconds":2.0,
        "changedSamplesAfterFrontEdit":front_edit,"changedSamplesAfterRearEdit":rear_edit,
        "byteIdenticalMovieFramesAtTwoSeconds":movie_matches
    });
    fs::write(
        dir.join("capture.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
