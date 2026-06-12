/* te2.h — C ABI over the RE-DEEMER tape engine (te2-dsp).
 *
 * Maintained BY HAND in lockstep with crates/te2-capi/src/lib.rs.
 * If anything here changes, bump TE2_ABI_VERSION in both files and check
 * te2_abi_version() at startup.
 *
 * Threading: te2_create/te2_destroy are NOT realtime-safe (creation runs
 * magnetics calibration). te2_process is realtime-safe; te2_set_params is
 * control-rate and realtime-safe.
 */
#ifndef TE2_H
#define TE2_H

#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#define TE2_ABI_VERSION 2u

typedef struct te2_handle te2_handle;

/* Enum mappings:
 *   tape_kind:        0 = I (normal), 1 = II (chrome), 2 = IV (metal)
 *   stock:            0 Maxell XL-II, 1 TDK SA, 2 TDK MA, 3 Sony Metal-ES,
 *                     4 BASF Chrome Maxima, 5 Nakamichi EX-II, 6 TDK AD,
 *                     7 Maxell UD-II, 8 Sony UX, 9 TDK D, 10 Sony HF,
 *                     11 Realistic Supertape, 12 Memorex, 13 no-name ferric
 *   os_factor:        0 = 2x, 1 = 4x, 2 = 8x
 *   white_target:     0 time, 1 resonance, 2 mod speed
 *   gray_target:      0 feedback, 1 mod amount, 2 LPF
 *   black_target:     0 tape level, 1 dry level, 2 HPF
 *   anomaly_polarity: -1 minus, 0 off, 1 plus
 *   transport:        0 echo, 1 play, 2 loop
 *   wind:             0 off, 1 rewind, 2 fast-forward
 */
typedef struct te2_params {
    float delay_time;      /* seconds, 0.06..1.5 */
    float feedback;        /* 0..1.5 (>1 runs away) */
    float tape_in;         /* gain into tape, 1.0 = unity */
    float tape_level;
    float dry_level;
    float mod_amount;      /* 0..1 */
    float mod_speed_hz;    /* 0.1..150 */
    bool motor_kill;
    float hpf_hz;          /* 20..2000 */
    float lpf_hz;          /* 100..18000 */
    float res;             /* 0..1, self-osc from ~0.93 */
    float out_drive;       /* 0..1 */
    int32_t tape_kind;
    int32_t stock;
    bool aging_on;
    bool aging_freeze;
    float condition;       /* 0 mint .. 1 wreck */
    float noise_amount;    /* 1.0 = calibrated hiss */
    int32_t os_factor;

    float white_faders[7]; /* positions 2..8, normalized 0..1 */
    float gray_faders[7];
    float black_faders[7];
    bool white_on;
    bool gray_on;
    bool black_on;
    int32_t white_target;
    int32_t gray_target;
    int32_t black_target;
    float white_drift;     /* seconds, 0..14 */
    float gray_drift;
    float black_drift;
    bool cycle_run;
    int32_t cycle_len;     /* 1..8 */
    float cycle_rate;      /* steps per second, 0.125..4000 */
    bool host_step_valid;
    double host_step_pos;  /* absolute song position in steps */
    int32_t manual_position; /* 1..8 */
    float anomaly_amount;  /* 0..1 */
    int32_t anomaly_polarity;
    bool res_gate_enabled;
    bool gate_held;
    int32_t transport;
    bool pause;
    bool stop;
    int32_t wind;
    float loop_len_s;      /* 0.5..30 */
    float slip;            /* slip-clutch drag 0..1 (irregular sag); ABI v2 */
    bool external_clock;   /* cycle steps only via te2_clock_step(); ABI v2 */
} te2_params;

uint32_t te2_abi_version(void);
/* sizeof(te2_params) as the library sees it — assert it matches yours. */
size_t te2_params_size(void);

/* Returns NULL on an absurd sample rate. */
te2_handle *te2_create(double sample_rate);
void te2_destroy(te2_handle *h);

void te2_default_params(te2_params *out);
void te2_set_params(te2_handle *h, const te2_params *p);

/* One stereo frame. */
void te2_process(te2_handle *h, float in_l, float in_r,
                 float *out_l, float *out_r);

void te2_eject(te2_handle *h);
void te2_reset(te2_handle *h);

/* Advance the cycle one step (external clock; realtime-safe). */
void te2_clock_step(te2_handle *h);
/* True once per cycle final-step entry (EOC trigger source). */
bool te2_take_eoc(te2_handle *h);
/* The three sets' drift-slewed values, normalized 0..1 (Set CV outputs).
 * Output pointers may be NULL to skip. */
void te2_set_values(const te2_handle *h, float *white, float *gray, float *black);
/* Static label of a tape stock (header order), e.g. "MAXELL XL-II". */
const char *te2_stock_label(int32_t index);
int32_t te2_stock_count(void);

float te2_vu(const te2_handle *h);
double te2_motor_speed(const te2_handle *h);
int32_t te2_position(const te2_handle *h);
float te2_age(const te2_handle *h);
void te2_set_age(te2_handle *h, float age);
double te2_footage_seconds(const te2_handle *h);

#ifdef __cplusplus
}
#endif

#endif /* TE2_H */
