/* Moving cut-paper aperture and an independent travelling glass matte for the media compositor. */
#include <cairo.h>
#include <math.h>
#include <stdio.h>
#include <string.h>

enum { width = 960, height = 540, frames = 72 };
static const double tau = 6.2831853071795864769;

static void star(cairo_t *cr, double radius, double angle) {
    for (int point = 0; point < 10; point++) {
        const double a = angle + point * tau / 10;
        const double r = (point % 2 ? 0.51 : 1.0) * radius;
        const double x = cos(a) * r, y = sin(a) * r;
        if (point == 0) cairo_move_to(cr, x, y);
        else cairo_line_to(cr, x, y);
    }
    cairo_close_path(cr);
}

static void aperture(cairo_t *cr, int frame) {
    const double phase = frame / (double)frames;
    const double dye[][3] = {{0.97,0.71,0.30}, {0.98,0.38,0.26}, {0.91,0.21,0.46}};
    /* Three successive bands pass each radius in one loop. */
    cairo_translate(cr, width*(0.52 + 0.035*cos(tau*phase)), height*(0.52 + 0.035*sin(tau*phase)));
    cairo_set_fill_rule(cr, CAIRO_FILL_RULE_EVEN_ODD);
    for (int ring = -3; ring <= 12; ring++) {
        const double step = ring + 3*phase;
        const double radius = 12 * pow(1.46, step);
        const double angle = -tau/4 + 0.095 * step + 0.08*sin(phase*tau);
        const double *colour = dye[(ring+3)%3];
        cairo_new_path(cr);
        star(cr, radius, angle);
        star(cr, radius*0.79, angle - 0.012);
        cairo_set_source_rgb(cr, colour[0], colour[1], colour[2]);
        cairo_fill(cr);
    }
}

static void matte(cairo_t *cr, int frame) {
    const double phase = frame*tau/frames;
    cairo_translate(cr, width*(0.46+0.16*cos(phase)), height*(0.50+0.12*sin(phase)));
    cairo_rotate(cr, -0.4+0.25*sin(phase));
    cairo_scale(cr, 268, 198);
    cairo_set_source_rgb(cr, 1, 1, 1);
    cairo_arc(cr, 0, 0, 1, 0, tau);
    cairo_fill(cr);
    cairo_set_operator(cr, CAIRO_OPERATOR_CLEAR);
    cairo_arc(cr, 0.37, -0.05, 0.42, 0, tau);
    cairo_fill(cr);
}

int main(int argc, char **argv) {
    if (argc != 3 || (strcmp(argv[1],"star") && strcmp(argv[1],"matte"))) {
        fprintf(stderr, "Usage: fixture star|matte existing-output-directory\n");
        return 2;
    }
    for (int i = 0; i < frames; i++) {
        cairo_surface_t *s = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, width, height);
        cairo_t *cr = cairo_create(s);
        if (strcmp(argv[1],"star") == 0) aperture(cr, i);
        else matte(cr, i);
        char path[4096];
        int length = snprintf(path, sizeof path, "%s/%03d.png", argv[2], i);
        if (length < 0 || length >= (int)sizeof path) return 2;
        cairo_status_t status = cairo_status(cr);
        if (status == CAIRO_STATUS_SUCCESS) status = cairo_surface_write_to_png(s, path);
        cairo_destroy(cr);
        cairo_surface_destroy(s);
        if (status != CAIRO_STATUS_SUCCESS) {
            fprintf(stderr, "%s: %s\n", path, cairo_status_to_string(status));
            return 1;
        }
    }
    return 0;
}
