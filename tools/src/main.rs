use clap::{Parser, ValueHint::EitherFileOrDir};
use std::io::{self, Write};

#[derive(Parser, Debug)]
#[command(author, version, about = "Send raw HID frames to a USB device")]
struct Args {
    /// USB Vendor ID (hex, e.g., 041e)
    #[arg(short, long)]
    vid: String,

    /// USB Product ID (hex, e.g., 329b)
    #[arg(short, long)]
    pid: String,

    /// Raw HID frame as hex string (e.g., 5a3903000e00)
    #[arg(short, long)]
    frame: String,

    /// Use libusb instead of hidapi
    #[arg(short, long, default_value = "false")]
    libusb: bool,
}

fn main() {
    let args = Args::parse();

    let vid = u16::from_str_radix(&args.vid, 16).expect("Invalid VID");
    let pid = u16::from_str_radix(&args.pid, 16).expect("Invalid PID");
    let frame = hex::decode(&args.frame.replace(" ", "")).expect("Invalid hex frame");

    println!("Sending to {:04x}:{:04x}: {:02x?}", vid, pid, frame);

    if args.libusb {
        send_libusb(vid, pid, &frame);
    } else {
        send_hidapi(vid, pid, &frame);
    }
}

fn send_hidapi(vid: u16, pid: u16, frame: &[u8]) {
    let devices = hidapi::enumerate(vid, pid);
    if devices.is_empty() {
        eprintln!("No HID devices found with VID:PID {:04x}:{:04x}", vid, pid);
        return;
    }

    let device = devices.into_iter().next().expect("Failed to open device");
    println!("Using hidapi backend");

    let report_id = frame.get(0).copied();
    let data = if report_id.is_some() {
        frame.to_vec()
    } else {
        // Prepend a zero report ID
        let mut v = vec![0x00];
        v.extend_from_slice(frame);
        v
    };

    let mut buf = vec![0u8; 64];
    buf[..data.len().min(64)].copy_from_slice(&data[..data.len().min(64)]);

    match device.send_feature_report(&buf) {
        Ok(_) => println!("Feature report sent successfully"),
        Err(e) => {
            // Try as an output report instead
            match device.write(&buf) {
                Ok(_) => println!("Output report sent successfully"),
                Err(e) => eprintln!("Failed to send: {}", e),
            }
        }
    }
}

fn send_libusb(vid: u16, pid: u16, frame: &[u8]) {
    let context = rusb::Context::new().expect("Failed to create USB context");
    let mut handle = context.open_device_with_vid_pid(vid, pid)
        .expect("Device not found")
        .take_on_release(true);

    println!("Using libusb backend");

    // For HID devices, we send via SET_REPORT control transfer
    // bmRequestType: 0x21 (host->device, class, interface)
    // bRequest: 0x09 (SET_REPORT)
    // wValue: report_type (0x03 for feature) | report_id (0x00)
    // wIndex: interface number
    // wLength: length of data
    let report_type = 0x03u16; // HID feature report
    let report_id = frame.get(0).copied().unwrap_or(0x00);
    let w_value = (report_type << 8) | report_id as u16;

    let data: Vec<u8> = if report_id == 0x00 && frame.len() > 0 && frame[0] != 0x00 {
        // If first byte isn't a report ID, prepend 0x00
        std::iter::once(0x00).chain(frame.iter().copied()).collect()
    } else {
        frame.to_vec()
    };

    // Ensure 64-byte minimum length with padding
    let mut buf = vec![0u8; 64];
    buf[..data.len().min(64)].copy_from_slice(&data[..data.len().min(64)]);

    match handle.write_control(0x21, 0x09, w_value, 0, &buf, std::time::Duration::from_secs(1)) {
        Ok(n) => println!("Sent {} bytes via SET_REPORT control transfer", n),
        Err(e) => eprintln!("Control transfer failed: {}", e),
    }
}
