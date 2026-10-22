// Input for the emulator's controller port.
//
// Phones and tablets: a paired Bluetooth controller (DualSense, DualShock,
// Xbox, MFi...) through the browser Gamepad API. There are deliberately no
// on-screen touch controls.
// Computers: the same controllers, plus keyboard and mouse.
//
// Button bit N matches the browser "standard" gamepad button N, which is also
// the DualSense layout: Cross, Circle, Square, Triangle, L1, R1, L2, R2,
// Create, Options, L3, R3, Up, Down, Left, Right, PS, Touchpad.

export const BUTTON_NAMES = [
  "Cross", "Circle", "Square", "Triangle", "L1", "R1", "L2", "R2", "Create", "Options",
  "L3", "R3", "Up", "Down", "Left", "Right", "PS", "Touchpad",
] as const;

export const SOURCE = { NONE: 0, CONTROLLER: 1, KEYBOARD_MOUSE: 2 } as const;

export interface InputFrame {
  source: number;
  buttons: number;
  leftX: number;
  leftY: number;
  rightX: number;
  rightY: number;
  l2: number;
  r2: number;
  /** Controller name as the browser reports it, or "" */
  controllerName: string;
}

const bit = (name: (typeof BUTTON_NAMES)[number]) => 1 << BUTTON_NAMES.indexOf(name);

// Keyboard layout (event.code -> button). WASD is the left stick.
export const KEY_BUTTONS: Record<string, (typeof BUTTON_NAMES)[number]> = {
  Space: "Cross",
  KeyC: "Circle",
  KeyF: "Square",
  KeyR: "Triangle",
  KeyQ: "L1",
  KeyE: "R1",
  ShiftLeft: "L3",
  KeyV: "R3",
  Backspace: "Create",
  Enter: "Options",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  KeyH: "PS",
  KeyT: "Touchpad",
};
const STICK_KEYS = new Set(["KeyW", "KeyA", "KeyS", "KeyD"]);

/** True on devices with a precise pointer (computers), false on phones/tablets. */
export const hasMouse = () => window.matchMedia("(pointer: fine)").matches;

const toByte = (v: number) => Math.max(0, Math.min(255, Math.round(((v + 1) / 2) * 255)));

export class InputManager {
  private keys = new Set<string>();
  private mouseDX = 0;
  private mouseDY = 0;
  private mouseButtons = 0;
  private usedKeyboardMouse = false;

  constructor(private screen: HTMLCanvasElement) {
    window.addEventListener("keydown", (e) => {
      if (e.target instanceof HTMLSelectElement || e.metaKey || e.ctrlKey) return;
      if (e.code in KEY_BUTTONS || STICK_KEYS.has(e.code)) {
        this.keys.add(e.code);
        this.usedKeyboardMouse = true;
        e.preventDefault(); // keep arrows/space from scrolling the page
      }
    });
    window.addEventListener("keyup", (e) => this.keys.delete(e.code));
    window.addEventListener("blur", () => {
      this.keys.clear();
      this.mouseButtons = 0;
    });

    // Mouse = right stick + triggers, only on computers, after clicking the screen.
    if (hasMouse()) {
      screen.addEventListener("click", () => {
        if (document.pointerLockElement !== screen) screen.requestPointerLock?.();
      });
      document.addEventListener("mousemove", (e) => {
        if (document.pointerLockElement !== screen) return;
        this.mouseDX += e.movementX;
        this.mouseDY += e.movementY;
        this.usedKeyboardMouse = true;
      });
      document.addEventListener("mousedown", (e) => {
        if (document.pointerLockElement !== screen) return;
        this.mouseButtons |= 1 << e.button;
        this.usedKeyboardMouse = true;
      });
      document.addEventListener("mouseup", (e) => {
        this.mouseButtons &= ~(1 << e.button);
      });
      screen.addEventListener("contextmenu", (e) => e.preventDefault());
    }
  }

  get mouseLocked(): boolean {
    return document.pointerLockElement === this.screen;
  }

  /** First connected controller, if any. Browsers only list one after a button press. */
  private gamepad(): Gamepad | null {
    const pads = navigator.getGamepads ? navigator.getGamepads() : [];
    for (const p of pads) if (p && p.connected) return p;
    return null;
  }

  /** Sample everything once; call once per frame. */
  read(): InputFrame {
    const f: InputFrame = {
      source: SOURCE.NONE, buttons: 0, leftX: 128, leftY: 128, rightX: 128, rightY: 128, l2: 0, r2: 0, controllerName: "",
    };

    const pad = this.gamepad();
    if (pad) {
      f.source = SOURCE.CONTROLLER;
      f.controllerName = pad.id;
      pad.buttons.forEach((b, i) => {
        if (i < BUTTON_NAMES.length && (b.pressed || b.value > 0.5)) f.buttons |= 1 << i;
      });
      if (pad.axes.length >= 4) {
        f.leftX = toByte(pad.axes[0]);
        f.leftY = toByte(pad.axes[1]);
        f.rightX = toByte(pad.axes[2]);
        f.rightY = toByte(pad.axes[3]);
      }
      f.l2 = Math.round((pad.buttons[6]?.value ?? 0) * 255);
      f.r2 = Math.round((pad.buttons[7]?.value ?? 0) * 255);
    }

    // Keyboard and mouse add on top (a controller's sticks win if one is connected).
    for (const code of this.keys) {
      const name = KEY_BUTTONS[code];
      if (name) f.buttons |= bit(name);
    }
    if (!pad) {
      const k = (c: string) => (this.keys.has(c) ? 1 : 0);
      f.leftX = toByte(k("KeyD") - k("KeyA"));
      f.leftY = toByte(k("KeyS") - k("KeyW"));
      // Mouse motion this frame pushes the right stick; it re-centers when the mouse stops.
      const gain = 0.08;
      f.rightX = toByte(Math.max(-1, Math.min(1, this.mouseDX * gain)));
      f.rightY = toByte(Math.max(-1, Math.min(1, this.mouseDY * gain)));
      if (this.mouseButtons & 1) {
        f.buttons |= bit("R2");
        f.r2 = 255;
      }
      if (this.mouseButtons & 4) {
        f.buttons |= bit("L2");
        f.l2 = 255;
      }
      if (this.usedKeyboardMouse) f.source = SOURCE.KEYBOARD_MOUSE;
    }
    this.mouseDX = 0;
    this.mouseDY = 0;
    return f;
  }
}

export const pressedNames = (buttons: number) => BUTTON_NAMES.filter((_, i) => buttons & (1 << i));
