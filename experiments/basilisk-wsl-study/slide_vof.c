/*
 * Single-plane confined-flow integration study.
 *
 * The pressure boundary conditions and material-mobility coefficient follow
 * Basilisk's upstream src/examples/hele-shaw.c. The initialized shapes are
 * scene inputs; pressure, velocity and volume-fraction advection are computed
 * by Basilisk's Hele-Shaw and VOF solvers. No optical effects are modeled here.
 * The forced left-to-right flow is solely a diagnostic, not a TSS default.
 *
 * This study does not establish capillary forces, calibrated coalescence,
 * plate compression, material properties or visual fidelity.
 */
#include "grid/multigrid.h"
#include "hele-shaw.h"
#include "vof.h"

scalar phase[];
scalar * interfaces = {phase};
scalar * tracers = NULL;

double mu1 = 1., mu2 = 0.2, k = 0.25;

p[left] = dirichlet (0.1);
p[right] = dirichlet (0.);

int main (void)
{
  size (1.);
  init_grid (256);
  DT = 0.05;
  run();
}

event init (i = 0)
{
  fraction (phase, max (sq(0.12) - sq(x - 0.29) - sq(y - 0.42),
                        sq(0.15) - sq(x - 0.62) - sq(y - 0.61)));
}

event coefficients (i++)
{
  foreach_face() {
    double ff = (phase[] + phase[-1])/2.;
    beta.x[] = -k/(mu1 + clamp(ff, 0, 1)*(mu2 - mu1));
  }
}

event frames (t += 0.25; t <= 5.)
{
  static int frame = 0;
  char name[64];
  snprintf (name, sizeof name, "frame-%03d.ppm", frame);
  FILE * fp = fopen (name, "wb");
  if (!fp) {
    perror (name);
    exit (1);
  }
  output_ppm (phase, fp = fp, n = 256, min = 0, max = 1, map = gray);
  fclose (fp);
  stats s = statsf (phase);
  fprintf (stderr, "%d,%.17g,%.17g,%.17g,%.17g\n",
           frame++, t, s.sum, s.min, s.max);
  fflush (stderr);
}

event end (t = 5.)
{
  return 1;
}
