# Check the numerical full-world phase integral and fraction bounds at every sample.
BEGIN { FS = "," }
/^frame,/ {
    if ($2 != frames || $5 < -1e-8 || $6 > 1 + 1e-8) bad = 1
    if (!frames) start = $4
    if (start <= 0 || start >= 1) exit 1
    error = ($4-start)/start
    if (error < 0) error = -error
    if (error > worst) worst = error
    frames++
}
END {
    printf "%s samples=%d initial_phase=%.12g max_relative_deviation=%.8g\n", FILENAME, frames, start, worst
    if (bad || frames != 96 || worst > 5e-5) exit 1
}
