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
#include <stdint.h>
/* qcc may translate on Linux and compile the emitted C on Windows. These
 * deferred directives use the target compiler and its standard headers. */
@if defined(__CYGWIN__) || defined(__MSYS__)
@include <fcntl.h>
@include <io.h>
@define TSS_BINARY_STDOUT() setmode(fileno(stdout), O_BINARY)
@elif defined(_WIN32)
@include <fcntl.h>
@include <io.h>
@define TSS_BINARY_STDOUT() _setmode(_fileno(stdout), _O_BINARY)
@else
@define TSS_BINARY_STDOUT() 0
@endif

enum { raster_limit = 1024, rate = 24 };
static int raster_width = raster_limit;
static int raster_height = 576, full_slide = 0;
static unsigned char pixels[raster_limit*raster_limit];
static unsigned char * initial;
static int iw, ih, slide, frame, total = 96, maxlevel = 9;
static const char * outdir;
static double slide_gap;
// Basilisk classifies the event increment before entering main().
static double frame_dt = 1./rate;

/* One process owns one sealed cell. The live client submits a displacement
 * per simulation tick, so pauses in wall time do not change the trajectory. */
static int live = 0;
static int driven = 0;
static coord contact, wall_velocity;
static double stroke_phase[4], stroke_sense;
typedef struct { coord center; double radius, load; } PlatePress;
static PlatePress held;
static int volume_audit;
static double flat_tolerance;
scalar cell_gap[];
face vector face_gap[];
typedef struct { double x0,y0,rx,ry,cosine,sine; } LiveDrop;
static LiveDrop drops[24];

static double unit_random (uint32_t * state) {
  *state = *state * UINT32_C(1664525) + UINT32_C(1013904223);
  return *state / 4294967296.;
}

/* Two broad, moving tangential tractions represent the operator slowly
 * working different parts of a compliant plate. They act on momentum; the
 * interface still moves by VOF advection and relaxes under surface tension. */
typedef struct { coord center; double radius, speed; } TangentialStroke;

static void seed_strokes (uint32_t seed) {
  seed ^= slide ? UINT32_C(0x62ec52e1) : UINT32_C(0x87bf7955);
  for (int i = 0; i < 4; ++i)
    stroke_phase[i] = 2.*pi*unit_random(&seed);
  stroke_sense = unit_random(&seed) < .5 ? -1. : 1.;
}

static void plate_strokes (double time, TangentialStroke strokes[2]) {
  for (int i = 0; i < 2; ++i) {
    double angle = stroke_phase[0] + i*pi +
      (i ? -.094 : .12)*time + .32*sin(.19*time + stroke_phase[1+i]);
    double arm = .17 + .032*cos(.17*time + stroke_phase[2+i]);
    strokes[i].center = (coord){.5 + arm*cos(angle), .5 + arm*sin(angle)};
    strokes[i].radius = i ? .43 : .48;
    strokes[i].speed = (i ? -.14 : .17)*stroke_sense*
      (.86 + .14*sin(.23*time + stroke_phase[2+i]));
  }
}

static coord stroke_velocity (const TangentialStroke strokes[2], double px, double py) {
  coord velocity = {0., 0.};
  for (int i = 0; i < 2; ++i) {
    double dx = (px-strokes[i].center.x)/strokes[i].radius;
    double dy = (py-strokes[i].center.y)/strokes[i].radius;
    double r2 = sq(dx) + sq(dy);
    if (r2 < 1.) {
      double omega = 4.*strokes[i].speed*sq(1.-r2);
      velocity.x -= omega*dy;
      velocity.y += omega*dx;
    }
  }
  return velocity;
}

static void seed_drops (uint32_t seed) {
  seed ^= slide ? UINT32_C(0xa511e9b3) : UINT32_C(0x63d83595);
  for (size_t j = 0; j < sizeof drops / sizeof drops[0]; ++j) {
    double cx = unit_random(&seed), cy = unit_random(&seed);
    double radius = j < 8 ? .085 + .095*unit_random(&seed) : .018 + .032*unit_random(&seed);
    double aspect = 1. + .85*unit_random(&seed);
    double angle = 2.*pi*unit_random(&seed);
    double fraction_scale = slide ? .84 : 1.;
    drops[j] = (LiveDrop){cx,cy,fraction_scale*radius*aspect,
                         fraction_scale*radius/aspect,cos(angle),sin(angle)};
  }
}

/* Positive inside the union of the initial oil drops. Basilisk constructs
 * its initial VOF fractions from this field; the shapes are never evaluated
 * during subsequent integration or rendering. */
static double seed_level_set (double px, double py) {
  double best = -HUGE;
  for (size_t j = 0; j < sizeof drops / sizeof drops[0]; ++j) {
    LiveDrop d = drops[j];
    double dx = px-d.x0, dy = py-d.y0;
    double s = (dx*d.cosine+dy*d.sine)/d.rx;
    double q = (dy*d.cosine-dx*d.sine)/d.ry;
    best = max(best, 1.-s*s-q*q);
  }
  return best;
}

static double brush (double xface, double yface) {
  double r2 = sq((xface-contact.x)/.16) + sq((yface-contact.y)/.16);
  return r2 < 1. ? sq(1.-r2) : 0.;
}

/* Time-integrated depth flux through a vertical face of the glass sandwich.
 * Its negative divergence gives h/h0 - 1 = -0.65 P (1-s)^2 (1-4s).
 * The positive annular gap change balances the center in the sealed cell. */
static coord plate_displacement (PlatePress p, double px, double py) {
  coord d = {px-p.center.x, py-p.center.y};
  double s = p.radius > 0. ? (sq(d.x) + sq(d.y))/sq(p.radius) : 1.;
  double k = s < 1. ? 0.325*p.load*cube(1.-s) : 0.;
  return (coord){k*d.x, k*d.y};
}

/* A donor PLIC slab carries oil through each face, with the remaining flux
 * carrying the other phase. Both cells use the same signed face integrals. */
foreach_dimension()
static void squeeze_sweep_x (face vector flux, double weight) {
  vector normal[];
  scalar intercept[];
  face vector oil[];
  reconstruction(f, normal, intercept);
  foreach_face(x) {
    double q = flux.x[]*weight;
    int donor = q > 0. ? -1 : 0;
    double c = f[donor];
    if (q && c > 0. && c < 1.) {
      double sign_q = q > 0. ? 1. : -1.;
      double width = fabs(q)/(Delta*cell_gap[donor]);
      c = rectangle_fraction((coord){-sign_q*normal.x[donor], normal.y[donor]},
                             intercept[donor], (coord){-.5,-.5}, (coord){width-.5,.5});
    }
    oil.x[] = q*c;
  }
  foreach() {
    double h = cell_gap[];
    double next_h = h + weight*(flux.x[] - flux.x[1])/Delta;
    f[] = (h*f[] + (oil.x[] - oil.x[1])/Delta)/next_h;
    cell_gap[] = next_h;
  }
}

static void squeeze (PlatePress next) {
  next.radius = min(next.radius, min(min(next.center.x, 1.-next.center.x),
                                     min(next.center.y, 1.-next.center.y)));
  if (next.radius == 0.) next.load = 0.;
  if (!held.load && !next.load) return;
  if (held.center.x == next.center.x && held.center.y == next.center.y &&
      held.radius == next.radius && held.load == next.load) return;

  if (!held.load) {
    /* Preserve the PLIC interface on refinement before changing cell
     * capacities. Use shared equal-sized faces while the gap is nonuniform. */
    refine(level < maxlevel);
    foreach() cell_gap[] = 1.;
    foreach_face() face_gap.x[] = 1.;
    cm = cell_gap;
    fm = face_gap;
    flat_tolerance = TOLERANCE;
    TOLERANCE = min(flat_tolerance, 1.e-6);
  }
  face vector flux[];
  double max_width = 0.;
  foreach_face(reduction(max:max_width)) {
    coord old_d = plate_displacement(held,x,y);
    coord new_d = plate_displacement(next,x,y);
    flux.x[] = new_d.x - old_d.x;
    max_width = max(max_width, fabs(flux.x[])/Delta);
  }
  /* At least 0.35 neutral depth remains under a full press. Keep each
   * swept slab well below half that capacity, including opposing outflux. */
  int steps = max(1, (int)ceil(max_width/.10));
  for (int sub = 0; sub < steps; ++sub) {
    squeeze_sweep_x(flux, 1./steps);
    squeeze_sweep_y(flux, 1./steps);
  }
  foreach_face() {
    double h = (cell_gap[] + cell_gap[-1])/2.;
    uf.x[] *= h/face_gap.x[];
    face_gap.x[] = h;
  }
  restriction((scalar *){cell_gap,face_gap});
  /* Geometry changed between fluid ticks. Project the reweighted basal flux
   * before the next advection so it obeys the new stationary-gap constraint. */
  event("properties");
  project(uf, pf, alpha, frame_dt);
  held = next;
  if (!held.load) {
    /* The discrete flux difference telescopes back to unit capacity. */
    cm = unity;
    fm = unityf;
    TOLERANCE = flat_tolerance;
  }
}

static int valid_press (double x, double y, double radius, double load) {
  return isfinite(x) && isfinite(y) && isfinite(radius) && isfinite(load) &&
    x >= 0. && x <= 1. && y >= 0. && y <= 1. &&
    radius >= 0. && radius <= 1. && load >= 0. && load <= 1.;
}

/* 0 = quit, 1 = normal fluid tick, 2 = squeeze only at the current time. */
static int live_command (void) {
  char line[256], extra;
  if (!fgets(line,sizeof line,stdin)) {
    if (ferror(stdin)) { perror("read live step"); exit(6); }
    return 0;
  }
  if (!strcmp(line,"QUIT\n") || !strcmp(line,"QUIT\r\n") || !strcmp(line,"QUIT")) return 0;
  double px,py,dx,dy,drive,force_x=0.,force_y=0.,radius=0.,load=0.;
  int end = 0;
  if (!strncmp(line,"PRESS ",6)) {
    if (sscanf(line+6,"%lf %lf %lf %lf %c",&force_x,&force_y,&radius,&load,&extra) != 4 ||
        !valid_press(force_x,force_y,radius,load)) {
      fprintf(stderr,"expected PRESS x y radius load\n"); exit(6);
    }
    squeeze((PlatePress){{force_x,1.-force_y},radius,load});
    return 2;
  }
  if (sscanf(line,"STEP %lf %lf %lf %lf %lf%n",&px,&py,&dx,&dy,&drive,&end) != 5 ||
      !isfinite(px) || !isfinite(py) || !isfinite(dx) || !isfinite(dy) || !isfinite(drive) ||
      px < 0. || px > 1. || py < 0. || py > 1. || fabs(dx)>1. || fabs(dy)>1. ||
      drive < 0. || drive > 1.) {
    fprintf(stderr,"expected STEP x y dx dy drive [press_x press_y radius load] or QUIT\n"); exit(6);
  }
  while (isspace((unsigned char)line[end])) ++end;
  if (line[end] && (sscanf(line+end,"%lf %lf %lf %lf %c",&force_x,&force_y,&radius,&load,&extra) != 4 ||
                    !valid_press(force_x,force_y,radius,load))) {
    fprintf(stderr,"invalid STEP pressure contact\n"); exit(6);
  }
  squeeze((PlatePress){{force_x,1.-force_y},radius,load});
  contact.x = px; contact.y = 1.-py;
  wall_velocity.x = drive*dx/frame_dt; wall_velocity.y = -drive*dy/frame_dt;
  return 1;
}

static int parse_uint (const char * text, uint32_t * result) {
  if (!*text) return 0;
  uint32_t value = 0;
  for (; *text; ++text) {
    if (*text < '0' || *text > '9') return 0;
    uint32_t digit = *text-'0';
    if (value > (UINT32_MAX-digit)/10) return 0;
    value = value*10+digit;
  }
  *result = value;
  return 1;
}

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
  driven = argc > 1 && !strcmp(argv[1],"--live-driven");
  live = driven || (argc > 1 && !strcmp(argv[1],"--live"));
  if (live) {
    uint32_t seed, level = 5;
    if ((argc != 4 && argc != 5) || (strcmp(argv[2],"violet") && strcmp(argv[2],"cyan")) ||
        !parse_uint(argv[3],&seed) || (argc == 5 && !parse_uint(argv[4],&level)) ||
        level < 5 || level > 8) {
      fprintf(stderr,"solver --live|--live-driven violet|cyan seed_u32 [level_5_to_8]\n"); return 2;
    }
    slide = !strcmp(argv[2],"cyan"); full_slide = 1; maxlevel = level;
    volume_audit = getenv("TSS_LIVE_VOLUME_AUDIT") != NULL;
    raster_height = raster_width = 1 << maxlevel;
    seed_drops(seed);
    if (driven) seed_strokes(seed);
    if (TSS_BINARY_STDOUT() < 0) { perror("stdout binary mode"); return 3; }
  } else {
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
  }
  rho1 = rho2 = 1.; mu1 = .00012; mu2 = .00018;
  f.sigma = .000035;
  double world_ratio = full_slide ? 1.375 : 1.;
  slide_gap = (slide ? .095 : .085)/world_ratio;
  frame_dt = 1./rate;
  DT = frame_dt;
  if (!live && argc == 15) {
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
  if (!live && frame != total) { fprintf(stderr,"incomplete sequence\n"); return 5; }
}

event init (i = 0) {
  if (live)
    fraction(f,seed_level_set(x,y));
  foreach() {
    if (!live) {
      double sum = 0.;
      for (int j = 0; j < 6; j++)
        for (int k = 0; k < 6; k++)
          sum += coverage(x + Delta*((k+.5)/6.-.5), y + Delta*((j+.5)/6.-.5));
      f[] = sum/36.;
    }
    double h = .5*Delta;
    u.x[] = (streamfunction(x,y+h)-streamfunction(x,y-h))/(2.*h);
    u.y[] = -(streamfunction(x+h,y)-streamfunction(x-h,y))/(2.*h);
  }
}

event adapt (i++) {
  if (!held.load)
    adapt_wavelet((scalar *){f,u}, (double[]){.012,.003,.003}, maxlevel, 5);
}

event acceleration (i++) {
  /* Depth-averaged no-slip plate drag -12 mu/(rho h^2) u. */
  face vector av = a;
  TangentialStroke strokes[2];
  if (driven) plate_strokes(t,strokes);
  foreach_face() {
    double phase = clamp((f[]+f[-1])/2.,0.,1.);
    double viscosity = phase*mu1+(1.-phase)*mu2;
    double gap = slide_gap*fm.x[];
    av.x[] -= 12.*viscosity/(rho(phase)*sq(gap)) * (u.x[]+u.x[-1])/2.;
    /* With one plate moving, the depth-mean Couette contribution is half
     * its local velocity. Pressure projection retains incompressibility. */
    if (live && (wall_velocity.x || wall_velocity.y))
      av.x[] += 6.*viscosity/(rho(phase)*sq(gap)) * brush(x,y)*wall_velocity.x;
    if (driven) {
      coord shear = stroke_velocity(strokes,x,y);
      av.x[] += 6.*viscosity/(rho(phase)*sq(gap)) * shear.x;
    }
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
  stats s = statsf(f);
  if (!isfinite(s.sum) || !isfinite(s.min) || !isfinite(s.max) ||
      s.min < -1e-8 || s.max > 1.+1e-8) { fprintf(stderr,"invalid VOF bounds\n"); exit(4); }
  size_t count = (size_t)raster_width*raster_height;
  if (live) {
    if (volume_audit) {
      double lo = HUGE, hi = 0.;
      foreach(reduction(min:lo) reduction(max:hi)) {
        lo = min(lo,cm[]); hi = max(hi,cm[]);
      }
      fprintf(stderr,"volume %d %.17g %.17g %.17g %.17g %.17g\n",
              frame, s.sum, s.volume-s.sum, s.volume, lo, hi);
    }
    if (printf("TSSVOF1 %d %.12g %d %d %.15g\n",frame,time,raster_width,raster_height,s.sum) < 0 ||
        fwrite(pixels,1,count,stdout) != count || fflush(stdout)) exit(3);
    return;
  }
  char path[1024]; snprintf(path,sizeof path,"%s/%03d.pgm",outdir,frame);
  FILE * fp = fopen(path,"wbx");
  if (!fp) { perror(path); exit(3); }
  fprintf(fp,"P5\n%d %d\n255\n",raster_width,raster_height);
  if (fwrite(pixels,1,count,fp)!=count || fclose(fp)) exit(3);
  fprintf(stderr,"frame,%d,%.8g,%.14g,%.6g,%.6g\n",frame,time,s.sum,s.min,s.max);
  fflush(stderr);
}
event frames (t = 0; live || t < (total - 0.5)*frame_dt; t += frame_dt) {
  do {
    emit_frame(t);
    ++frame;
    if (!live) return frame == total;
    int cmd = live_command();
    if (!cmd) return 1;
    if (cmd == 1) break;
  } while (1);
}
