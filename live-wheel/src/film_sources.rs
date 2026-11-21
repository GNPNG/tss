use super::{fields::Fields, FilmOil, MaterialMotion, MotionPreset};

/// Each RG8 solve stores its own rear geometry in R and front geometry in G.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FilmSource {
    Wheel,
    Coral,
    SmallCoral,
    Pdms20,
    Pdms1000,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct FilmFrame {
    pub source: FilmSource,
    pub index: u32,
}

impl From<FilmOil> for FilmSource {
    fn from(oil: FilmOil) -> Self {
        match oil {
            FilmOil::Wheel => Self::Wheel,
            FilmOil::Pdms20 => Self::Pdms20,
            FilmOil::Pdms1000 => Self::Pdms1000,
        }
    }
}

impl FilmSource {
    pub fn for_slides(preset: MotionPreset, oils: MaterialMotion) -> [Self; 2] {
        match preset {
            MotionPreset::WheelFlow => [oils.rear.into(), oils.front.into()],
            MotionPreset::WheelWithCoralBeads => [oils.rear.into(), Self::SmallCoral],
            MotionPreset::CoralRelease => [Self::Coral; 2],
        }
    }
    pub fn sample(self, timing: &Fields, seconds: f64) -> ([u32; 2], f32) {
        let motion = match self {
            Self::Coral | Self::SmallCoral => MotionPreset::CoralRelease,
            _ => MotionPreset::WheelFlow,
        };
        let (a, b, weight) = timing.sample_motion(motion, seconds);
        ([a, b], weight)
    }
}

#[derive(Default)]
pub(super) struct OilFields {
    pdms20: Option<Fields>,
    pdms1000: Option<Fields>,
}

impl OilFields {
    pub fn prepare(&mut self, sources: [FilmSource; 2], glass: &Fields) -> Result<(), String> {
        for source in sources {
            let (slot, gzip): (_, &[u8]) = match source {
                FilmSource::Pdms20 => (
                    &mut self.pdms20,
                    include_bytes!("../assets/oils/pdms-water-20cst.rg8.gz"),
                ),
                FilmSource::Pdms1000 => (
                    &mut self.pdms1000,
                    include_bytes!("../assets/oils/pdms-water-1000cst.rg8.gz"),
                ),
                _ => continue,
            };
            if !glass.registered_full_slide() {
                return Err(
                    "PDMS motion requires the registered full-slide geometry and 24 Hz timebase"
                        .into(),
                );
            }
            if slot.is_none() {
                let fields = Fields::from_gzip(gzip)?;
                if !fields.registered_full_slide() {
                    return Err(format!(
                        "Incorrect glass registration or timebase for {source:?}"
                    ));
                }
                *slot = Some(fields);
            }
        }
        Ok(())
    }
    pub fn fields(&self, source: FilmSource) -> Option<&Fields> {
        match source {
            FilmSource::Pdms20 => self.pdms20.as_ref(),
            FilmSource::Pdms1000 => self.pdms1000.as_ref(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::small_front::cache_slots;

    #[test]
    fn selecting_and_replaying_a_coral_front_preserves_the_authored_rear() {
        let oils = MaterialMotion {
            rear: FilmOil::Pdms20,
            front: FilmOil::Pdms1000,
        };
        assert_eq!(
            FilmSource::for_slides(MotionPreset::WheelFlow, oils),
            [FilmSource::Pdms20, FilmSource::Pdms1000]
        );
        assert_eq!(
            FilmSource::for_slides(MotionPreset::WheelWithCoralBeads, oils),
            [FilmSource::Pdms20, FilmSource::SmallCoral]
        );
        assert_eq!(
            FilmSource::for_slides(MotionPreset::CoralRelease, oils),
            [FilmSource::Coral; 2]
        );
        assert_eq!(
            FilmSource::for_slides(MotionPreset::WheelFlow, Default::default()),
            [FilmSource::Wheel; 2]
        );
    }

    #[test]
    fn changing_source_invalidates_matching_frame_numbers_on_one_film() {
        let mut slots = [[None; 2]; 2];
        let mut uploads = Vec::new();
        let mut sample = |film: usize, source, frames: [u32; 2]| {
            cache_slots(
                &mut slots[film],
                frames.map(|index| FilmFrame { source, index }),
                |slot, frame| uploads.push((film, slot, frame.source, frame.index)),
            )
        };
        sample(0, FilmSource::Pdms20, [18, 19]);
        sample(1, FilmSource::Wheel, [18, 19]);
        sample(1, FilmSource::Pdms1000, [18, 19]);
        sample(0, FilmSource::Pdms20, [19, 20]);
        sample(1, FilmSource::Pdms1000, [18, 19]);
        assert_eq!(uploads.len(), 7);
        assert_eq!(
            &uploads[4..],
            &[
                (1, 0, FilmSource::Pdms1000, 18),
                (1, 1, FilmSource::Pdms1000, 19),
                (0, 0, FilmSource::Pdms20, 20)
            ]
        );
    }

    #[test]
    fn registered_oil_fields_step_independently_and_retain_both_physical_masks() {
        let wheel = Fields::from_gzip(include_bytes!("../assets/slide-world.rg8.gz")).unwrap();
        let mut oils = OilFields::default();
        let [rear, front] = FilmSource::for_slides(
            MotionPreset::WheelFlow,
            MaterialMotion {
                rear: FilmOil::Pdms1000,
                front: FilmOil::Pdms20,
            },
        );
        oils.prepare([rear, front], &wheel).unwrap();
        let thick = oils.fields(rear).unwrap();
        let thin = oils.fields(front).unwrap();
        assert!(
            thick.frame(0) == thin.frame(0),
            "matched starts across viscosity grades"
        );
        let changed = |a: &Fields, af, ac: usize, b: &Fields, bf, bc: usize| {
            a.frame(af)
                .chunks_exact(2)
                .zip(b.frame(bf).chunks_exact(2))
                .filter(|(p, q)| (p[ac] >= 128) != (q[bc] >= 128))
                .count()
        };
        // Distinct physical masks for either grade and measurable motion for
        // each of the four solves, including a viscosity change on either film.
        for f in [thick, thin] {
            assert!(changed(f, 0, 0, f, 0, 1) > 1000);
            for slide in [0, 1] {
                assert!(changed(f, 0, slide, f, 80, slide) > 100);
            }
        }
        for slide in [0, 1] {
            assert!(changed(thick, 80, slide, thin, 80, slide) > 100);
        }
        assert_eq!(rear.sample(&wheel, 1.), ([24, 25], 0.));
        assert_eq!(front.sample(&wheel, 2.03125), ([48, 49], 0.75));
        // A completed front coral take holds while the rear viscosity solve retraces.
        assert_eq!(FilmSource::SmallCoral.sample(&wheel, 5.), ([95, 95], 0.));
        assert_eq!(rear.sample(&wheel, 5.), ([70, 71], 0.));
    }
}
