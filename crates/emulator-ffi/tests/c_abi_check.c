/* Links against the static library through include/emulator.h exactly as the
 * iOS app will, and runs the reference programs. Built and run by
 * crates/emulator-ffi/tests/c_abi.sh. */
#include "emulator.h"
#include <stdio.h>
#include <string.h>

static void print_result(const char *label, size_t len) {
    printf("%s%.*s\n", label, (int)len, (const char *)emu_result_ptr());
}

int main(void) {
    print_result("core v", emu_core_version());

    size_t len = emu_run_all_tests();
    const char *json = (const char *)emu_result_ptr();
    int any_failed = memmem(json, len, "\"passed\":false", 14) != NULL;
    printf("reference programs: %zu, JSON %zu bytes, failures: %s\n",
           emu_test_program_count(), len, any_failed ? "YES" : "none");

    /* x86-64 bytes: mov rax,7 ; add rax,5 ; hlt */
    const uint8_t code[] = {0x48, 0xC7, 0xC0, 7, 0, 0, 0, 0x48, 0x83, 0xC0, 5, 0xF4};
    MachineHandle *m = emu_machine_new(code, sizeof code);
    int st, steps = 0;
    while ((st = emu_machine_step(m)) == EMU_RUNNING) steps++;
    size_t n = emu_machine_state_json(m);
    int ok = st == EMU_HALTED &&
             memmem(emu_result_ptr(), n, "[\"rax\",\"0x000000000000000c\"]", 27) != NULL;
    printf("step API: status %d after %d steps, rax==12: %s\n", st, steps + 1, ok ? "yes" : "NO");
    emu_machine_free(m);

    return (any_failed || !ok) ? 1 : 0;
}
