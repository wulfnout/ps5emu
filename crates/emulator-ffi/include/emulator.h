/*
 * C interface to the emulator core (crates/emulator-ffi).
 *
 * Used by the iOS app through a Swift bridging header / module map, and
 * mirrors the WebAssembly exports the browser loader calls.
 *
 * Strings: functions documented as "returns a result length" place UTF-8 text
 * (not NUL-terminated) in a per-thread buffer; read it at emu_result_ptr().
 * It stays valid until the next result-producing call on the same thread.
 */
#ifndef EMULATOR_H
#define EMULATOR_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct MachineHandle MachineHandle;

enum {
    EMU_RUNNING = 0,
    EMU_HALTED  = 1,
    EMU_ERROR   = 2,
};

const uint8_t *emu_result_ptr(void);

uint8_t *emu_alloc(size_t len);
void     emu_free(uint8_t *ptr, size_t len);

size_t emu_core_version(void);        /* returns a result length */
size_t emu_run_all_tests(void);       /* returns a result length (JSON array) */
size_t emu_test_program_count(void);

MachineHandle *emu_machine_new(const uint8_t *code, size_t len); /* NULL if too large */
MachineHandle *emu_machine_new_test_program(size_t index);       /* NULL if no such program */
void           emu_machine_free(MachineHandle *h);

uint32_t emu_screen_width(void);
uint32_t emu_screen_height(void);
int32_t  emu_test_program_draws(size_t index);
/* width*height*4 bytes, 0x00RRGGBB little-endian, valid until the machine is freed */
const uint8_t *emu_machine_screen_ptr(const MachineHandle *h);

/* One input frame. source: 0 none, 1 controller, 2 keyboard/mouse.
 * buttons: bit order Cross, Circle, Square, Triangle, L1, R1, L2, R2, Create,
 * Options, L3, R3, Up, Down, Left, Right, PS, Touchpad. Sticks/triggers 0..255. */
void emu_machine_set_input(MachineHandle *h, uint32_t source, uint32_t buttons,
                           uint32_t left_x, uint32_t left_y, uint32_t right_x, uint32_t right_y,
                           uint32_t l2, uint32_t r2);

int32_t emu_machine_step(MachineHandle *h);              /* EMU_* status */
int32_t emu_machine_run(MachineHandle *h);               /* EMU_* status */
int32_t emu_machine_run_steps(MachineHandle *h, uint32_t n); /* at most n instructions */
size_t  emu_machine_state_json(const MachineHandle *h);  /* returns a result length (JSON) */

#ifdef __cplusplus
}
#endif

#endif /* EMULATOR_H */
