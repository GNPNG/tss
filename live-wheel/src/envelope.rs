use serde::{Deserialize, Serialize};

/// A timeline edit may crop a region while retaining the original fade curve.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpacityRegion {
    pub start: f64,
    pub end: f64,
    pub fade_in: f64,
    pub fade_out: f64,
    pub level: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_start: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_end: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(try_from = "Vec<OpacityRegion>")]
pub struct OpacityEnvelope(Vec<OpacityRegion>);

impl TryFrom<Vec<OpacityRegion>> for OpacityEnvelope {
    type Error = String;
    fn try_from(regions: Vec<OpacityRegion>) -> Result<Self, String> {
        let mut previous_end = 0.;
        for r in &regions {
            let left = r.source_start.unwrap_or(r.start);
            let right = r.source_end.unwrap_or(r.end);
            if ![left, right, r.start, r.end, r.fade_in, r.fade_out]
                .iter()
                .all(|n| n.is_finite())
                || !(0.0..=1.0).contains(&r.level)
                || r.start < previous_end
                || left > r.start
                || r.start >= r.end
                || r.end > right
                || !(0.0..=right - left).contains(&r.fade_in)
                || !(0.0..=right - left).contains(&r.fade_out)
            {
                return Err("Wheel opacity regions require ordered, disjoint intervals, valid source bounds, fades and levels".into());
            }
            previous_end = r.end;
        }
        Ok(Self(regions))
    }
}

impl OpacityEnvelope {
    pub fn level_at(&self, time: f64) -> f32 {
        let index = self.0.partition_point(|r| r.end <= time);
        let Some(r) = self
            .0
            .get(index)
            .filter(|r| time >= r.start && time < r.end)
        else {
            return 0.;
        };
        let left = r.source_start.unwrap_or(r.start);
        let right = r.source_end.unwrap_or(r.end);
        let fade_in = if r.fade_in > 0. {
            (time - left) / r.fade_in
        } else {
            1.
        };
        let fade_out = if r.fade_out > 0. {
            (right - time) / r.fade_out
        } else {
            1.
        };
        r.level * fade_in.min(fade_out).min(1.) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn cropped_fades_and_disjoint_half_open_regions_follow_song_time() {
        let intervals = json!([
            {"start":2,"end":4,"sourceStart":0,"sourceEnd":8,"fadeIn":4,"fadeOut":2,"level":0.8},
            {"start":6,"end":7.5,"sourceStart":0,"sourceEnd":8,"fadeIn":4,"fadeOut":2,"level":0.8},
            {"start":8,"end":9,"fadeIn":0,"fadeOut":0,"level":0.3}]);
        let envelope: OpacityEnvelope = serde_json::from_value(intervals.clone()).unwrap();
        assert_eq!(serde_json::to_value(&envelope).unwrap()[0]["sourceEnd"], 8.);
        for (time, level) in [
            (1., 0.),
            (2., 0.4),
            (3., 0.6),
            (4., 0.),
            (6., 0.8),
            (7., 0.4),
            (7.5, 0.),
            (8., 0.3),
            (9., 0.),
        ] {
            assert!((envelope.level_at(time) - level).abs() < 1e-6);
        }
        assert_eq!(
            OpacityEnvelope::try_from(Vec::new()).unwrap().level_at(4.),
            0.
        );
        let cropped_at_song_start: OpacityEnvelope = serde_json::from_value(json!([{
            "start":0,"end":1,"sourceStart":-1,"sourceEnd":1,"fadeIn":2,"fadeOut":0,"level":1
        }]))
        .unwrap();
        assert_eq!(cropped_at_song_start.level_at(0.), 0.5);
        for (index, key, value) in [
            (1, "start", 3.),
            (0, "sourceStart", 3.),
            (0, "sourceEnd", 3.),
            (2, "fadeOut", 2.),
            (1, "level", 1.2),
        ] {
            let mut bad = intervals.clone();
            bad[index][key] = json!(value);
            assert!(serde_json::from_value::<OpacityEnvelope>(bad).is_err());
        }
    }
}
