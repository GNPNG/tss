# Compare samples within each track. Interleaving across tracks may differ.
def disagreements($ours; $reference; $field):
  [range(0; ([$ours, $reference] | map(length) | min)) as $n
   | select($ours[$n][$field] != $reference[$n][$field])] | length;
def samples($list; $idx): [$list[] | select(.stream_index == $idx)];
def ts_base($s): ($s | split("/") | map(tonumber));
$probe_file[0] as $probe | $ff_file[0] as $ff |
if ($probe.streams | map(.index) | sort) != ($ff.streams | map(.index) | sort)
then error("stream indices differ") else . end |
[$probe.streams[] as $s
 | ($ff.streams[] | select(.index == $s.index)) as $f
 | samples($probe.packets; $s.index) as $o | samples($ff.packets; $s.index) as $r
 | {index: $s.index, identifier: [$s.codec,$f.codec_name],
    dimensions: [$s.size,[$f.width,$f.height]], audio: [$s.audio,[(if $f.sample_rate then ($f.sample_rate | tonumber) else null end),$f.channels]],
    time_base_match: ($s.time_base == ts_base($f.time_base)),
    duration_ticks: [$s.duration, $f.duration_ts], start_ticks: [$s.start,$f.start_pts],
    color: {oxide: [$s.primaries,$s.transfer,$s.matrix,$s.range], ffprobe: [$f.color_primaries,$f.color_transfer,$f.color_space,$f.color_range]},
    pixel_format: [$s.pixel_format,$f.pix_fmt], packets: [($o|length),($r|length)],
    paired_packet_differences: {payload_sha256: disagreements($o;$r;"data_hash"), pts: disagreements($o;$r;"pts"),
                                dts: disagreements($o;$r;"dts"), duration: disagreements($o;$r;"duration")}}]
