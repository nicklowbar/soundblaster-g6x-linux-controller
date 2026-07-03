# USB Send Utility

A command-line tool for sending raw HID frames to USB devices.

## Building

```bash
cargo build --release
```

The binary will be at `target/release/usb_send`.

## Usage

### With hidapi (default)

```bash
usb_send --vid 041e --pid 329b "5a3903000e00"
```

### With libusb

```bash
usb_send --vid 041e --pid 329b --libusb "5a3903000e00"
```

## Options

- `--vid`, `-v`: USB Vendor ID (hex)
- `--pid`, `-p`: USB Product ID (hex)
- `--frame`, `-f`: Raw HID frame as hex string (spaces optional)
- `--libusb`, `-l`: Use libusb backend instead of hidapi

## Examples

Send an LED on command to a Sound Blaster device:

```bash
usb_send -v 041e -p 329b "5a3903000e01"
```

Send an LED off command:

```bash
usb_send -v 041e -p 329b "5a3903000e00"
```

## Backends

- **hidapi** (default): Uses the hidapi library, which works with kernel HID drivers
- **libusb**: Direct USB access via libusb, requires no kernel driver but device must be claimed
