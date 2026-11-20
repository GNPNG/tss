/* Single-viscosity, dimensionless squeeze-film reference for one closed slide. */
#include "grid/multigrid.h"
#include "poisson.h"

scalar pressure[], gap_rate[];
face vector mobility[];
pressure[left] = dirichlet(0.);
pressure[right] = dirichlet(0.);
pressure[top] = dirichlet(0.);
pressure[bottom] = dirichlet(0.);

static const double radius = 0.30;

static double gap_shape (double s) {
  return s < 1. ? sq(1. - s)*(1. - 4.*s) : 0.;
}

/* Integrate -dp/dr from the axis to the edge, using s = (r/R)^2. */
static double center_pressure (double amplitude, double rate) {
  const int samples = 8192;
  double integral = 0.;
  for (int i = 0; i < samples; i++) {
    double s = (i + 0.5)/samples;
    integral += cube((1. - s)/(1. - amplitude*gap_shape(s)));
  }
  return sq(radius)*rate*integral/(4.*samples);
}

static double solve (int cells, double amplitude, double rate) {
  init_grid(cells);
  foreach() {
    double s = (sq(x) + sq(y))/sq(radius);
    pressure[] = 0.;
    gap_rate[] = -rate*gap_shape(s);
  }
  foreach_face() {
    double s = (sq(x) + sq(y))/sq(radius);
    mobility.x[] = cube(1. - amplitude*gap_shape(s));
  }
  mgstats mg = poisson(pressure, gap_rate, mobility, tolerance = 1e-9);
  boundary({pressure});

  double err2 = 0., ref2 = 0.;
  foreach_face(reduction(+:err2) reduction(+:ref2)) {
    double s = (sq(x) + sq(y))/sq(radius);
    coord position = {x, y};
    double exact = s < 1. ? rate*position.x*cube(1. - s)/2. : 0.;
    double flux = -mobility.x[]*(pressure[] - pressure[-1])/Delta;
    err2 += sq(flux - exact);
    ref2 += sq(exact);
  }
  double error = sqrt(err2/(ref2 > 0. ? ref2 : 1.));
  double axis = interpolate(pressure, 0., 0.);
  double reference = center_pressure(amplitude, rate);
  double peak_error = reference ? fabs(axis/reference - 1.) : fabs(axis);
  printf("%d,%.2f,%+.1f,%d,%.5e,%.7e,%.7e,%.7e,%.7e\n",
         cells, amplitude, rate, mg.i, mg.resa, error, axis, reference, peak_error);
  free_grid();
  if (!(mg.resa <= 1e-9 && peak_error < 0.09))
    return HUGE_VAL;
  return error;
}

int main() {
  size(1. [0]);
  origin(-0.5, -0.5);
  puts("cells,amplitude,ramp_rate,cycles,residual,relative_flux_l2,axis_pressure,analytic_axis,relative_axis_error");
  for (int direction = -1; direction <= 1; direction += 2) {
    double previous = 1.;
    for (int n = 64; n <= 512; n *= 2) {
      double error = solve(n, 0.65, direction);
      if (!(error < previous*0.35) || (n == 512 && error > 2e-4))
        return 1;
      previous = error;
    }
  }
  return solve(128, 0.65, 0.) == 0. ? 0 : 1;
}
