/*
 * One fully occupied, periodic 2D binary-liquid domain.  Basilisk's centered
 * Navier–Stokes, two-phase VOF and surface-tension modules provide the physics;
 * this file only supplies initial shapes, material values and output events.
 * No air/free surface, gravity, wall-gap model or imposed flow is configured.
 * The two ellipses are initial conditions, not animated image assets.
 *
 * This is an integration study, not a calibrated glass-slide simulation.
 * Plate compression, optical projection and physically validated merger timing
 * are not modeled. Numerical coalescence is not evidence of physical drainage.
 */
#include "grid/multigrid.h"
#include "navier-stokes/centered.h"
#include "two-phase.h"
#include "tension.h"

int main (void)
{
  size (1.);
  periodic (right);
  periodic (top);
  init_grid (128);
  rho1 = rho2 = 1.;
  mu1 = mu2 = 0.02;
  f.sigma = 0.2;
  DT = 0.01;
  run();
}

event init (t = 0)
{
  fraction (f, max (1. - sq((x - 0.31)/0.16) - sq((y - 0.43)/0.09),
                   1. - sq((x - 0.67)/0.13) - sq((y - 0.60)/0.09)));
}

event frames (t += 0.1; t <= 2.)
{
  static int frame = 0;
  char name[64];
  snprintf (name, sizeof name, "frame-%03d.ppm", frame);
  FILE * fp = fopen (name, "wb");
  if (!fp) {
    perror (name);
    exit (1);
  }
  output_ppm (f, fp = fp, n = 256, min = 0, max = 1, map = gray);
  fclose (fp);
  stats s = statsf (f);
  fprintf (stderr, "%d,%.17g,%.17g,%.17g,%.17g\n",
           frame++, t, s.sum, s.min, s.max);
  fflush (stderr);
}

event end (t = 2.)
{
  return 1;
}
