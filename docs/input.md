# Controller input

## Supported

| Device | Phone / tablet | Computer |
| --- | --- | --- |
| DualSense (PS5), DualShock 4, Xbox, MFi and other Bluetooth/USB controllers | Yes, via the browser Gamepad API | Yes |
| Keyboard and mouse | No | Yes |
| On-screen touch controls | Deliberately none | None |

Pairing is done by the operating system, not the page: on iPhone, hold
**PS + Create** on the DualSense until the light flashes, then pick it in
Settings → Bluetooth. Browsers only report a controller to a page after its
first button press.

Not available through the browser Gamepad API: the DualSense touchpad
surface, motion sensors, adaptive triggers, haptics and the light bar. The
touchpad *click* is available (button 17).

## Controller port (guest view)

Mapped read-only at `0x200000`. All fields are little-endian 32-bit.

| Offset | Field |
| --- | --- |
| 0x00 | Frame counter — the host increments it once per frame |
| 0x04 | Source: 0 none, 1 controller, 2 keyboard/mouse |
| 0x08 | Buttons bitmask (below) |
| 0x0C / 0x10 | Left stick X / Y, 0–255, 128 = centered |
| 0x14 / 0x18 | Right stick X / Y, 0–255, 128 = centered |
| 0x1C / 0x20 | L2 / R2 analog, 0–255 |

Button bits 0–17: Cross, Circle, Square, Triangle, L1, R1, L2, R2, Create,
Options, L3, R3, Up, Down, Left, Right, PS, Touchpad. This is the same order
as the browser "standard" gamepad mapping, so browser button N is bit N.

This port is the emulator's own test interface. Real PS5 games read the pad
through Sony's system libraries, which the emulator does not implement yet;
when it does, those calls will be served from this same state.

## Keyboard and mouse (computers)

| Input | Maps to |
| --- | --- |
| WASD | Left stick |
| Mouse movement (click the screen first; Esc releases) | Right stick |
| Left / right click | R2 / L2 |
| Space, C, F, R | Cross, Circle, Square, Triangle |
| Q, E | L1, R1 |
| Left Shift, V | L3, R3 |
| Arrow keys | D-pad |
| Enter, Backspace | Options, Create |
| H, T | PS, Touchpad click |
