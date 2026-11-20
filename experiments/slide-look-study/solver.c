/*
 * Two immiscible phases occupy a closed square computational domain. Run a
 * separate process for each optical slide. Basilisk supplies adaptive geometric
 * VOF, surface tension, viscosity and the incompressible velocity/pressure solve.
 * The central 16:9 window is visible. Its data are reflected outside the crop
 * at initialization to allow pools to continue beyond the top and bottom.
 */
#include "grid/quadtree.h"
#include "navier-stokes/centered.h"
#include "two-phase.h"
#include "tension.h"
#include <ctype.h>

enum { raster_width = 1024, rate = 24 };
static int raster_height = 576, full_slide = 0;
static unsigned char pixels[raster_width*raster_width];
static unsigned char * initial;
static int iw, ih, slide, frame, total = 96, maxlevel = 9;
static const char * outdir;
static double slide_gap;
// Basilisk classifies the event increment before entering main().
static double frame_dt = 1./rate;

static int positive_number (const char * text, double * value) {
  char * end;
  *value = strtod(text, &end);
  return end != text && *end == 0 && isfinite(*value) && *value > 0.;
}

static int token (FILE * fp, char * text, size_t cap) {
  int c;
  do {
    c = fgetc (fp);
    if (c == '#') while (c != '\n' && c != EOF) c = fgetc (fp);
  } while (c != EOF && isspace(c));
  size_t n = 0;
  while (c != EOF && !isspace(c)) {
    if (n + 1 >= cap) return 0;
    text[n++] = c;
    c = fgetc(fp);
  }
  text[n] = 0;
  if (c == '\r') { c = fgetc (fp); if (c != '\n' && c != EOF) ungetc(c,fp); }
  return n > 0;
}

static int load_pgm (const char * path) {
  char s[64];
  FILE * fp = fopen(path, "rb");
  if (!fp) return 0;
  if (!token(fp,s,sizeof s) || strcmp(s,"P5") ||
      !token(fp,s,sizeof s) || (iw = atoi(s)) < 1 || iw > 8192 ||
      !token(fp,s,sizeof s) || (ih = atoi(s)) < 1 || ih > 8192 ||
      !token(fp,s,sizeof s) || strcmp(s,"255") || (full_slide ? iw != ih : iw*9 != ih*16)) {
    fclose(fp); return 0;
  }
  size_t count = (size_t)iw*ih;
  initial = malloc(count);
  int ok = initial && fread(initial,1,count,fp) == count;
  fclose(fp);
  return ok;
}

static double coverage (double world_x, double world_y) {
  double xx = (world_x-X0)/L0, yy = (world_y-Y0)/L0;
  if (full_slide) {
    int xi = clamp((int)(xx*iw), 0, iw-1);
    int yi = clamp((int)((1.-yy)*ih), 0, ih-1);
    return initial[yi*iw+xi]/255.;
  }
  double row = (0.5 - yy)*16./9. + 0.5;
  if (row < 0.) row = -row;
  if (row >= 1.) row = 2. - row;
  int xi = clamp((int)(xx*iw), 0, iw - 1);
  int yi = clamp((int)(row*ih), 0, ih - 1);
  return initial[yi*iw + xi] / 255.;
}

/* Offset vortex pairs produce different starting gestures on either sheet.
 * A sin^2 envelope gives zero in-plane velocity at the outer walls. */
static double streamfunction (double world_x, double world_y) {
  double nx = (world_x-X0)/L0, ny = (world_y-Y0)/L0;
  double xx = full_slide ? .5 + 1.375*(nx-.5) : nx;
  double yy = full_slide ? .5 + 1.375*(ny-.5) : ny;
  double cx = slide ? .56 : .39, cy = slide ? .45 : .55;
  double sx = slide ? .24 : .26, sy = slide ? .26 : .23;
  double sign = slide ? -1. : 1.;
  double a = exp(-sq((xx-cx)/sx)-sq((yy-cy)/sy));
  double b = exp(-sq((xx-(1.-cx))/.27)-sq((yy-(1.-cy))/.20));
  return sign*.018*sq(sin(pi*nx))*sq(sin(pi*ny))*(a - .48*b);
}

int main (int argc, char ** argv) {
  // Coordinates and time in the solve use reference length and velocity units.
  size(1. [0]);
  if (argc > 1 && !strcmp(argv[argc-1], "--world")) {
    full_slide = 1;
    raster_height = raster_width;
    --argc;
  }
  if (argc < 4 || (argc > 6 && argc != 15) || (strcmp(argv[3],"violet") && strcmp(argv[3],"cyan"))) {
    fprintf(stderr,"solver mask.pgm EMPTY_OUTDIR violet|cyan [count] [level] [--si width_m speed_m_s gap_m rho1_kg_m3 mu1_Pa_s rho2_kg_m3 mu2_Pa_s sigma_N_m]\n");
    return 2;
  }
  if (!load_pgm(argv[1])) { fprintf(stderr,"invalid P5 mask or aspect ratio for selected slide extent\n"); return 2; }
  outdir = argv[2]; slide = !strcmp(argv[3],"cyan");
  if (argc > 4) total = atoi(argv[4]);
  if (argc > 5) maxlevel = atoi(argv[5]);
  if (total < 1 || total > 240 || maxlevel < 7 || maxlevel > 9) return 2;
  rho1 = rho2 = 1.; mu1 = .00012; mu2 = .00018;
  f.sigma = .000035;
  double world_ratio = full_slide ? 1.375 : 1.;
  slide_gap = (slide ? .095 : .085)/world_ratio;
  frame_dt = 1./rate;
  DT = frame_dt;
  if (argc == 15) {
    if (strcmp(argv[6], "--si")) return 2;
    double v[8];
    for (int i = 0; i < 8; ++i)
      if (!positive_number(argv[7+i], &v[i])) return 2;
    double length = v[0]*world_ratio, speed = v[1], ref_density = v[5];
    if (v[2] >= length) return 2;
    slide_gap = v[2]/length;
    rho1 = v[3]/ref_density; rho2 = 1.;
    mu1 = v[4]/(ref_density*speed*length);
    mu2 = v[6]/(ref_density*speed*length);
    f.sigma = v[7]/(ref_density*sq(speed)*length);
    frame_dt = speed/(length*rate);
    if (!isfinite(rho1) || !isfinite(mu1) || !isfinite(mu2) ||
        !isfinite(f.sigma) || !isfinite(frame_dt) || frame_dt <= 0. ||
        -TEND_EVENT + frame_dt == -TEND_EVENT) {
      fprintf(stderr, "SI scales outside the solver event clock range\n");
      return 2;
    }
    // The plate drag is explicit. Bound its step for the most viscous phase.
    DT = min(frame_dt, sq(slide_gap)/(24.*max(mu1/rho1,mu2/rho2)));
    fprintf(stderr, "SI model rho*=%.9g,%.9g mu*=%.9g,%.9g sigma*=%.9g gap*=%.9g dt_frame*=%.9g\n",
            rho1, rho2, mu1, mu2, f.sigma, slide_gap, frame_dt);
  }
  init_grid(1 << maxlevel);
  run();
  free(initial);
  if (frame != total) { fprintf(stderr,"incomplete sequence\n"); return 5; }
}

event init (i = 0) {
  foreach() {
    double sum = 0.;
    for (int j = 0; j < 6; j++)
      for (int k = 0; k < 6; k++)
        sum += coverage(x + Delta*((k+.5)/6.-.5), y + Delta*((j+.5)/6.-.5));
    f[] = sum/36.;
    double h = .5*Delta;
    u.x[] = (streamfunction(x,y+h)-streamfunction(x,y-h))/(2.*h);
    u.y[] = -(streamfunction(x+h,y)-streamfunction(x-h,y))/(2.*h);
  }
}

event adapt (i++) {
  adapt_wavelet((scalar *){f,u}, (double[]){.012,.003,.003}, maxlevel, 5);
}

event acceleration (i++) {
  /* Depth-averaged no-slip plate drag -12 mu/(rho h^2) u.
   * The illustrative nondimensional gap h is fixed on each slide. */
  face vector av = a;
  foreach_face() {
    double phase = clamp((f[]+f[-1])/2.,0.,1.);
    double viscosity = phase*mu1+(1.-phase)*mu2;
    av.x[] -= 12.*viscosity/(rho(phase)*sq(slide_gap)) * (u.x[]+u.x[-1])/2.;
  }
}

static void emit_frame (double time) {
  vector normals[];
  scalar alpha[];
  reconstruction(f,normals,alpha);
  foreach(serial) {
    int x0 = (int)lround((x-Delta*.5)*raster_width);
    int y0 = (int)lround((y-Delta*.5)*raster_width) - (raster_width-raster_height)/2;
    int size_px = (int)lround(Delta*raster_width);
    if (y0+size_px <= 0 || y0 >= raster_height) continue;
    coord normal = {normals.x[],normals.y[]};
    for (int j=max(0,y0); j<min(raster_height,y0+size_px); j++) {
      unsigned char * row = pixels + (raster_height-1-j)*raster_width;
      if (f[] <= 0. || f[] >= 1.) {
        memset(row+x0,f[] >= 1. ? 255 : 0,size_px);
      } else {
        for (int k=x0; k<x0+size_px; k++) {
          coord lo = {(k-x0)/(double)size_px-.5,(j-y0)/(double)size_px-.5};
          coord hi = {lo.x+1./size_px,lo.y+1./size_px};
          row[k] = lround(255.*clamp(rectangle_fraction(normal,alpha[],lo,hi),0.,1.));
        }
      }
    }
  }
  char path[1024]; snprintf(path,sizeof path,"%s/%03d.pgm",outdir,frame);
  FILE * fp = fopen(path,"wbx");
  if (!fp) { perror(path); exit(3); }
  fprintf(fp,"P5\n%d %d\n255\n",raster_width,raster_height);
  size_t count = (size_t)raster_width*raster_height;
  if (fwrite(pixels,1,count,fp)!=count || fclose(fp)) exit(3);
  stats s = statsf(f);
  if (!isfinite(s.sum) || !isfinite(s.min) || !isfinite(s.max) ||
      s.min < -1e-8 || s.max > 1.+1e-8) { fprintf(stderr,"invalid VOF bounds\n"); exit(4); }
  fprintf(stderr,"frame,%d,%.8g,%.14g,%.6g,%.6g\n",frame,time,s.sum,s.min,s.max);
  fflush(stderr);
}
event frames (t = 0; t < (total - 0.5)*frame_dt; t += frame_dt) {
  emit_frame(t);
  if (++frame == total) return 1;
}
