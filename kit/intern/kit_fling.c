/* See kit_fling.h. The numbers and formulas are Android's (frameworks/base OverScroller.java,
 * frameworks/native VelocityTracker.cpp, ViewConfiguration.java).
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#include <math.h>
#include <string.h>

#include "kit_fling.h"

/* VelocityTracker */
#define HORIZON_MS 100.0
#define ASSUME_POINTER_STOPPED_MS 40.0

/* ViewConfiguration */
#define MINIMUM_FLING_DP 50.0f
#define MAXIMUM_FLING_DP 8000.0f
#define SCROLL_FRICTION 0.015f

/* SplineOverScroller */
#define INFLEXION 0.35f
#define START_TENSION 0.5f
#define END_TENSION 1.0f
#define P1 (START_TENSION * INFLEXION)
#define P2 (1.0f - END_TENSION * (1.0f - INFLEXION))
#define NB_SAMPLES 100

void bk_vt_clear(BKVelocityTracker *vt)
{
  vt->count = 0;
}

void bk_vt_add(BKVelocityTracker *vt, double t_ms, float x, float y)
{
  if (vt->count == BK_VT_HISTORY) {
    memmove(vt->t, vt->t + 1, sizeof(vt->t[0]) * (BK_VT_HISTORY - 1));
    memmove(vt->x, vt->x + 1, sizeof(vt->x[0]) * (BK_VT_HISTORY - 1));
    memmove(vt->y, vt->y + 1, sizeof(vt->y[0]) * (BK_VT_HISTORY - 1));
    vt->count--;
  }
  const int i = vt->count++;
  vt->t[i] = t_ms;
  vt->x[i] = x;
  vt->y[i] = y;
}

/* The slope at the newest touch of the quadratic through (age in seconds, position), the touches
 * not older than the horizon: solveUnweightedLeastSquaresDeg2 (a line when only two are left). */
static bool slope(const BKVelocityTracker *vt, const float *pos, int first, float *r_v)
{
  const int n = vt->count - first;
  if (n < 2) {
    return false;
  }
  const double newest = vt->t[vt->count - 1];
  float sxi = 0, sxi2 = 0, sxiyi = 0, syi = 0, sxi3 = 0, sxi2yi = 0, sxi4 = 0;
  for (int i = first; i < vt->count; i++) {
    const float xi = (float)((vt->t[i] - newest) * 0.001); /* <= 0 */
    const float yi = pos[i];
    const float xi2 = xi * xi;
    sxi += xi;
    sxi2 += xi2;
    sxiyi += xi * yi;
    syi += yi;
    sxi3 += xi2 * xi;
    sxi2yi += xi2 * yi;
    sxi4 += xi2 * xi2;
  }
  const float count = (float)n;
  const float Sxx = sxi2 - sxi * sxi / count;
  const float Sxy = sxiyi - sxi * syi / count;
  if (n == 2) { /* degree = size - 1 */
    if (Sxx == 0.0f) {
      return false;
    }
    *r_v = Sxy / Sxx;
    return true;
  }
  const float Sxx2 = sxi3 - sxi * sxi2 / count;
  const float Sx2y = sxi2yi - sxi2 * syi / count;
  const float Sx2x2 = sxi4 - sxi2 * sxi2 / count;
  const float denominator = Sxx * Sx2x2 - Sxx2 * Sxx2;
  if (denominator == 0.0f) {
    return false;
  }
  *r_v = (Sxy * Sx2x2 - Sx2y * Sxx2) / denominator;
  return true;
}

bool bk_vt_velocity(const BKVelocityTracker *vt, double now_ms, float *r_vx, float *r_vy)
{
  if (vt->count < 2 || now_ms - vt->t[vt->count - 1] > ASSUME_POINTER_STOPPED_MS) {
    return false;
  }
  int first = vt->count - 1; /* the oldest touch within the horizon */
  while (first > 0 && vt->t[vt->count - 1] - vt->t[first - 1] <= HORIZON_MS) {
    first--;
  }
  return slope(vt, vt->x, first, r_vx) && slope(vt, vt->y, first, r_vy);
}

/* SPLINE_POSITION: the distance covered (0..1) as the spline's time (0..1) advances, from the
 * same bisection OverScroller.java runs once in a static initializer. */
static float g_spline_position[NB_SAMPLES + 1];
static int g_spline_ready;

static void spline_init(void)
{
  float x_min = 0.0f;
  for (int i = 0; i < NB_SAMPLES; i++) {
    const float alpha = (float)i / NB_SAMPLES;
    float x_max = 1.0f, x, coef;
    for (;;) {
      x = x_min + (x_max - x_min) / 2.0f;
      coef = 3.0f * x * (1.0f - x);
      const float tx = coef * ((1.0f - x) * P1 + x * P2) + x * x * x;
      if (fabsf(tx - alpha) < 1e-5f) {
        break;
      }
      if (tx > alpha) {
        x_max = x;
      }
      else {
        x_min = x;
      }
    }
    g_spline_position[i] = coef * ((1.0f - x) * START_TENSION + x) + x * x * x;
  }
  g_spline_position[NB_SAMPLES] = 1.0f;
  g_spline_ready = 1;
}

bool bk_fling_start(BKFling *f, float velocity, float density)
{
  const float minv = MINIMUM_FLING_DP * density, maxv = MAXIMUM_FLING_DP * density;
  float v = fabsf(velocity);
  if (v < minv) {
    return false;
  }
  v = fminf(v, maxv);
  /* mPhysicalCoeff: g * inches per metre * ppi * 0.84 */
  const double ppi = (double)density * 160.0;
  const double coeff = 9.80665 * 39.37 * ppi * 0.84;
  const double deceleration_rate = log(0.78) / log(0.9);
  const double l = log(INFLEXION * v / (SCROLL_FRICTION * coeff)); /* getSplineDeceleration */
  const double decel_minus_one = deceleration_rate - 1.0;
  f->distance_px = (float)(SCROLL_FRICTION * coeff * exp(deceleration_rate / decel_minus_one * l));
  f->duration_ms = (float)(1000.0 * exp(l / decel_minus_one));
  f->distance_px = velocity < 0.0f ? -f->distance_px : f->distance_px;
  return f->duration_ms > 0.0f;
}

float bk_fling_at(const BKFling *f, double elapsed_ms, bool *r_done)
{
  if (!g_spline_ready) {
    spline_init();
  }
  const float t = (float)(elapsed_ms / f->duration_ms);
  *r_done = t >= 1.0f;
  if (t <= 0.0f) {
    return 0.0f;
  }
  if (*r_done) {
    return f->distance_px;
  }
  const int index = (int)(NB_SAMPLES * t);
  const float t_inf = (float)index / NB_SAMPLES, t_sup = (float)(index + 1) / NB_SAMPLES;
  const float d_inf = g_spline_position[index], d_sup = g_spline_position[index + 1];
  const float velocity_coef = (d_sup - d_inf) / (t_sup - t_inf);
  return (d_inf + (t - t_inf) * velocity_coef) * f->distance_px;
}
